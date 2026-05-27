// Lilia · Agent SDK 子进程包装器（双 backend：Claude / Codex）
//
// 调用约定（NDJSON 双向）：
//   - 父进程（Tauri Rust 端）启动 `node agent-runner.mjs`
//   - stdin 第一行：初始命令 JSON
//       {
//         "backend": "claude" | "codex",
//         "cwd": "...",
//         "prompt": "...",
//         "model": "...",
//         "resumeSessionId": "...|null",
//         "permission": "full|ask|readonly"
//       }
//   - stdin 后续行：父进程对此前发出的 control_request 的响应，目前包括：
//       {"type":"consent_response","id":"consent-1","decision":"allow"|"deny","message":"..."}
//       {"type":"ask_user_response","id":"ask-1","result":{...AskUserResult}}
//     父进程不会再关闭 stdin —— SDK 调 query 结束、runner 主动 exit 收尾。
//   - stdout 还是一行一条 NDJSON：
//       {"type":"timeline","event":{...}}         事实流（落到 UI 时间线）
//       {"type":"consent_request","id":"...",...} canUseTool 触发，等待父进程响应
//       {"type":"ask_user_request","id":"...","spec":{...AskUserSpec}} 等待用户反馈
//       {"type":"done","sessionId":"...","subtype":"success|error_..."}
//       {"type":"error","message":"..."}
//
// 关键约定：timeline 事件只输出「事实」字段 —— kind / status / title / summary /
// payload / sourceId。**绝不**再向 event 里塞 display：display 是渲染时的视图
// 缓存，前端 / `@lilia/contracts` 的 `deriveTimelineDisplay` 现算。改 display
// 规则可以即时影响历史数据，runner 不应固化任何中文文案或图标。
//
// 不是为长连接设计——每次发送都起一个新进程。Node 启动 + SDK 加载 ~200ms 是
// 当前可以接受的延迟代价；后续可以换成持久子进程多路复用，但协议不变。
//
// 隐藏 env：LILIA_AGENT_DRY_RUN=1 时跳过真实 SDK，模拟一段 NDJSON——用于单测
// agent 调度链路而不消耗真实 API。

import { createSdkMcpServer, query, tool } from "@anthropic-ai/claude-agent-sdk";
import { normalizeClaudeTool } from "@lilia/contracts/claudeTools.mjs";
import { createInterface } from "node:readline";
import { z } from "zod/v4";
import {
  createClaudeStreamState,
  dispatchClaudeStreamEvent,
  finalizeClaudeReasoningBlocks,
} from "./agent-runner/claudeStream.mjs";

const TIMELINE_RESERVED_KEYS = new Set([
  "taskId",
  "task_id",
  "turnId",
  "turn_id",
  "order",
  "thinking",
  "redacted_thinking",
  "signature",
]);

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function stringOrNull(value) {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return null;
}

function shortText(value, max = 600) {
  const text = stringOrNull(value);
  if (!text) return null;
  return text.length > max ? `${text.slice(0, max)}...` : text;
}

function toJsonSafe(value, seen = new WeakSet()) {
  if (
    value === undefined ||
    typeof value === "function" ||
    typeof value === "symbol"
  ) {
    return undefined;
  }
  if (typeof value === "bigint") return value.toString();
  if (value === null || typeof value !== "object") return value;
  if (value instanceof Error) {
    return {
      name: value.name,
      message: value.message,
      stack: value.stack,
    };
  }
  if (seen.has(value)) return "[Circular]";

  seen.add(value);
  if (Array.isArray(value)) {
    const safeArray = value
      .map((item) => toJsonSafe(item, seen))
      .filter((item) => item !== undefined);
    seen.delete(value);
    return safeArray;
  }

  const safeObject = {};
  for (const [key, item] of Object.entries(value)) {
    const safeItem = toJsonSafe(item, seen);
    if (safeItem !== undefined) safeObject[key] = safeItem;
  }
  seen.delete(value);
  return safeObject;
}

function sanitizeTimelinePayload(value, seen = new WeakSet(), depth = 0) {
  if (
    value === undefined ||
    typeof value === "function" ||
    typeof value === "symbol"
  ) {
    return undefined;
  }
  if (typeof value === "bigint") return value.toString();
  if (value === null || typeof value !== "object") return value;
  if (value instanceof Error) {
    return {
      name: value.name,
      message: value.message,
      stack: value.stack,
    };
  }
  if (depth > 5) return "[Truncated]";
  if (seen.has(value)) return "[Circular]";

  seen.add(value);
  if (Array.isArray(value)) {
    const safeArray = value
      .map((item) => sanitizeTimelinePayload(item, seen, depth + 1))
      .filter((item) => item !== undefined);
    seen.delete(value);
    return safeArray;
  }

  const safeObject = {};
  for (const [key, item] of Object.entries(value)) {
    if (TIMELINE_RESERVED_KEYS.has(key)) continue;
    const safeItem = sanitizeTimelinePayload(item, seen, depth + 1);
    if (safeItem !== undefined) safeObject[key] = safeItem;
  }
  seen.delete(value);
  return safeObject;
}

function emit(obj) {
  let line;
  try {
    line = JSON.stringify(obj);
  } catch {
    line = JSON.stringify(toJsonSafe(obj));
  }
  process.stdout.write(line + "\n");
}

function emitTimeline(input) {
  if (!input || typeof input !== "object") return;
  const kind = stringOrNull(input.kind);
  if (!kind) return;

  const status = stringOrNull(input.status) || "info";
  const title = shortText(input.title, 200) || kind;
  const summary = shortText(input.summary, 1200) || "";
  const payload = sanitizeTimelinePayload(input.payload);
  const sourceId = stringOrNull(input.sourceId);
  const event = {
    kind,
    status,
    title,
    summary,
    payload: payload === undefined ? {} : payload,
  };
  if (sourceId) event.sourceId = sourceId;
  emit({ type: "timeline", event });
}

/**
 * 把 assistant 流式回复按 **block 级** 拆成多条 inline message timeline 事件，
 * 每个 text block 用独立 sourceId（`{sessionId}:text:{blockKey}`）。
 * 这样每段文本固定落在它实际发生的位置，与同 turn 的工具/思考按 order 自然交错；
 * 旧的「整 turn 拼成单条 sourceId="assistant"」会让开场文本把整张回复卡的 order
 * 锁在工具之前。
 */
function emitAssistantTextFragmentTimeline(text, status, sessionId, blockKey) {
  const content = typeof text === "string" ? text : "";
  emitTimeline({
    kind: "message",
    status,
    title: "Assistant",
    summary: content,
    payload: {
      role: "assistant",
      content,
      backend: "claude",
    },
    sourceId: claudeTextFragmentSourceId(sessionId, blockKey),
  });
}

function claudeTextFragmentSourceId(sessionId, blockKey) {
  return `${sessionId || "claude"}:text:${blockKey}`;
}

/**
 * "假流式" 派发器：上游 SDK 实测按 1-2s 节奏成批 flush stream_event，
 * 直接转发会让 UI 看到"块状"刷新。这里把到达的 chunk 推进 buffer，按 tick
 * 节奏（默认 30Hz、约每 6 tick 排空一批）从 buffer 取一段提交，UI 看到的
 * 就是平稳滚动的文本流。
 *
 * 终态调 `finishImmediate` 一次性提交剩余，上层紧接 emit success，保证
 * 最终态不被 pacer 延迟。`syncTo` 给只发完整 snapshot 不发增量 delta 的
 * 上游补差量用（如 Codex 的 assistant_message item）。
 */
function createTextPacer({ intervalMs = 33, flushDivisor = 6, emit }) {
  let buffer = "";
  let committed = "";
  let timer = null;

  const tick = () => {
    if (!buffer) {
      clearInterval(timer);
      timer = null;
      return;
    }
    const take = Math.max(1, Math.ceil(buffer.length / flushDivisor));
    committed += buffer.slice(0, take);
    buffer = buffer.slice(take);
    emit(committed);
  };

  return {
    push(delta) {
      if (!delta) return;
      buffer += delta;
      if (!timer) timer = setInterval(tick, intervalMs);
    },
    finishImmediate() {
      if (timer) {
        clearInterval(timer);
        timer = null;
      }
      if (buffer) {
        committed += buffer;
        buffer = "";
        emit(committed);
      }
    },
    syncTo(fullText) {
      if (typeof fullText !== "string") return;
      if (fullText.length <= committed.length) return;
      const extra = fullText.slice(committed.length);
      buffer += extra;
      if (!timer) timer = setInterval(tick, intervalMs);
    },
  };
}

/**
 * Cumulative-snapshot 节流器：每次 push 整段累计文本，按 intervalMs 节奏吐出最新一帧。
 * 给 reasoning 这种「每条 delta 都是全量快照」的来源用——这样到达 Rust 那一侧的
 * timeline emit 间隔总是 ≥ intervalMs，Rust 的 throttle 永远走 due 分支立即落库，
 * 不会再出现 pending 卡住等不到唤醒的烂局。
 *
 * 配 33ms 默认，跟 text pacer 一档；`cancel` 放弃 pending 但不 emit，用在 caller
 * 紧接着自己 emit 终态（status='success'）的场景，避免一前一后两条相同 sourceId
 * 的 emit 互相覆盖。
 */
function createSnapshotPacer({ intervalMs = 33, emit }) {
  let pending = null;
  let lastEmitAt = 0;
  let timer = null;

  const flush = () => {
    timer = null;
    if (pending === null) return;
    const snapshot = pending;
    pending = null;
    lastEmitAt = Date.now();
    emit(snapshot);
  };

  return {
    push(snapshot) {
      pending = snapshot;
      if (timer) return;
      const elapsed = Date.now() - lastEmitAt;
      if (elapsed >= intervalMs) flush();
      else timer = setTimeout(flush, intervalMs - elapsed);
    },
    finishImmediate() {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
      if (pending !== null) {
        const snapshot = pending;
        pending = null;
        lastEmitAt = Date.now();
        emit(snapshot);
      }
    },
    cancel() {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
      pending = null;
    },
  };
}

function emitError(message, payload) {
  const text = shortText(message, 1200) || "unknown error";
  emit({ type: "error", message: text });
  emitTimeline({
    kind: "error",
    status: "error",
    title: "Error",
    summary: text,
    payload,
  });
}

function normalizePromptAttachments(value) {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => {
      if (!isRecord(item)) return null;
      const path = stringOrNull(item.path);
      if (!path) return null;
      return {
        name: stringOrNull(item.name) || path,
        path,
        kind: stringOrNull(item.kind) || "unknown",
        size: typeof item.size === "number" && Number.isFinite(item.size)
          ? item.size
          : null,
      };
    })
    .filter(Boolean);
}

function attachmentSizeLabel(size) {
  if (typeof size !== "number" || !Number.isFinite(size)) return "unknown size";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${Math.round(size / 1024)} KB`;
  return `${Math.round(size / (1024 * 1024))} MB`;
}

function buildPromptWithAttachments(prompt, attachments) {
  const normalized = normalizePromptAttachments(attachments);
  const content = typeof prompt === "string" ? prompt : "";
  if (normalized.length === 0) return content;
  const lines = [
    "用户随本轮消息附加的本地路径如下。不要假设已经读取了内容；需要时请使用可用工具按路径读取文件或遍历目录。",
    ...normalized.map((attachment, index) =>
      `${index + 1}. ${attachment.name} (${attachment.kind}, ${attachmentSizeLabel(attachment.size)}): ${attachment.path}`,
    ),
  ];
  const trimmedContent = content.trim();
  return trimmedContent
    ? `${lines.join("\n")}\n\n用户消息：\n${trimmedContent}`
    : lines.join("\n");
}

// ---------- 用户同意（canUseTool）双向通道 ----------
//
// SDK 在 "ask"/"default" 模式下对敏感工具调用走 canUseTool。这里把请求转成
// 一条 stdout NDJSON（`consent_request`），等父进程把决策写回 stdin
// （`consent_response`）。请求按递增 id 关联。
//
// 父进程在 turn 异常退出时直接关 stdin/杀进程——pending 的 Promise 不需要显式
// 拒绝，runner 整体会被 SIGTERM 终结。

const consentPending = new Map();
let consentSeq = 1;
const askUserPending = new Map();
let askUserSeq = 1;

function requestUserConsent(payload) {
  const id = `consent-${consentSeq++}`;
  emit({ type: "consent_request", id, ...payload });
  return new Promise((resolve) => {
    consentPending.set(id, resolve);
  });
}

function requestAskUser(spec) {
  const id = `ask-${askUserSeq++}`;
  emit({ type: "ask_user_request", id, spec });
  return new Promise((resolve) => {
    askUserPending.set(id, resolve);
  });
}

function handleControlLine(line) {
  let msg;
  try {
    msg = JSON.parse(line);
  } catch {
    return;
  }
  if (!isRecord(msg)) return;
  if (msg.type === "consent_response") {
    const resolve = consentPending.get(msg.id);
    if (!resolve) return;
    consentPending.delete(msg.id);
    resolve({
      decision: msg.decision === "allow" ? "allow" : "deny",
      message: stringOrNull(msg.message) || "",
    });
    return;
  }
  if (msg.type === "ask_user_response") {
    const resolve = askUserPending.get(msg.id);
    if (!resolve) return;
    askUserPending.delete(msg.id);
    resolve(normalizeAskUserResult(msg.result));
  }
}

/**
 * 给 Claude SDK 用的 canUseTool 实现。把 SDK 传来的 prompt 字段
 * （title / description / displayName / blockedPath / decisionReason / toolUseID）
 * 透传给父进程，让 UI 用最合适的文案渲染。
 */
async function claudeCanUseTool(toolName, input, opts) {
  const safeInput = input ?? {};
  const { decision, message } = await requestUserConsent({
    toolName,
    input: safeInput,
    toolUseID: stringOrNull(opts?.toolUseID),
    title: stringOrNull(opts?.title),
    displayName: stringOrNull(opts?.displayName),
    description: stringOrNull(opts?.description),
    blockedPath: stringOrNull(opts?.blockedPath),
    decisionReason: stringOrNull(opts?.decisionReason),
  });
  if (decision === "allow") {
    // Claude Code 二进制端 Zod 校验要求 allow 必须带 updatedInput——SDK 的 d.ts
    // 把它标成 optional 但底层 schema 实际 required。不填会被当作工具调用失败，
    // Agent 收到 is_error 结果会无限重试 canUseTool。原样回填即可。
    return { behavior: "allow", updatedInput: safeInput };
  }
  return { behavior: "deny", message: message || "用户拒绝了此次工具调用" };
}

function normalizeTimelineStatus(status) {
  switch (status) {
    case "failed":
      return "error";
    case "completed":
      return "success";
    case "in_progress":
      return "running";
    default:
      return status || "info";
  }
}

function fullTextOrNull(value) {
  return typeof value === "string" && value.length > 0 ? value : null;
}

// ---------- Claude ----------

function mapClaudePermission(p) {
  // Lilia 的三档语义 → Claude SDK 的 PermissionMode。
  // - full：直接放行所有工具调用，不弹窗。SDK 要求显式 opt-in。
  // - ask：SDK 默认行为；敏感工具走 canUseTool 回调（见 claudeCanUseTool），
  //        我们把请求通过 stdout 转交父进程，由 Lilia AskUser 浮层收用户决策。
  // - readonly：plan 模式，禁止写入。
  switch (p) {
    case "full":
      return { permissionMode: "bypassPermissions", allowDangerouslySkipPermissions: true };
    case "readonly":
      return { permissionMode: "plan" };
    case "ask":
    default:
      return { permissionMode: "default" };
  }
}

/**
 * 平台/Shell 提示：Claude Agent SDK 1.0 默认不注入任何 system prompt，模型就
 * 看不见自己在哪个平台跑、Bash 工具背后是什么 shell。Windows 上 Claude Code 的
 * Bash 工具走 Git Bash（/usr/bin/bash），不认 PowerShell cmdlet——若模型自作主张
 * 发 `Get-ChildItem | Select-Object` 就会 exit 127。这里启用 claude_code 预设
 * 拿回 SDK 内置的环境描述，并在 Windows 上 append 一段显式约束。
 */
function buildClaudePlatformAppend() {
  if (process.platform !== "win32") return "";
  return [
    "运行平台：Windows（win32）。",
    "Bash 工具在 Windows 上走 Git Bash 的 /usr/bin/bash，仅认 POSIX 命令，不认 PowerShell cmdlet（Get-ChildItem / Select-Object / Format-Table 等）。",
    "- 默认使用 POSIX 命令：ls / cat / grep / find / awk / sed / head / tail …",
    '- 必须用 PowerShell 时显式包装：powershell.exe -NoProfile -Command "<原 PS 命令> | Out-String"。',
    "- 路径用正斜杠或在双引号内用反斜杠都可。",
  ].join("\n");
}

function buildClaudeSystemPrompt() {
  const append = buildClaudePlatformAppend();
  const base = { type: "preset", preset: "claude_code" };
  return append ? { ...base, append } : base;
}

function claudeReasoningSourceId(sessionId, blockKey) {
  // 用 dispatcher 全局递增的 blockKey，而不是 SDK 每个 LLM turn 都从 0 重置的
  // content block index——否则第二个 turn 的 thinking 块会与第一个 turn 撞同一
  // sourceId / DB id，命中 insert 的 ON CONFLICT 路径继承旧 order，
  // 让后续 turn 的思考事件全部排到时间线最开头。
  return `${sessionId || "claude"}:thinking:${blockKey}`;
}

/** 单条 reasoning timeline emit 原语；status='success' 是终态，Rust throttle 会立即落库。 */
function emitClaudeReasoningTimeline(text, status, sessionId, blockKey) {
  emitTimeline({
    kind: "reasoning",
    status,
    title: "思考中",
    summary: text,
    payload: {
      backend: "claude",
      sessionId,
      text,
    },
    sourceId: claudeReasoningSourceId(sessionId, blockKey),
  });
}

/**
 * 拿 / 新建一个 thinking block 的累计快照 pacer。runner 把每条 onReasoning
 * 累计文本喂给它，pacer 按 33ms 节奏吐快照——到达 Rust 的 timeline emit 自然
 * 都隔够间隔，throttle 永远走 due 立即落库，UI 看到的就是平稳滚动的"正在思考"。
 */
function getOrCreateClaudeReasoningBlock(ctx, blockKey, sessionId) {
  let entry = ctx.reasoningBlocks.get(blockKey);
  if (entry) return entry;
  const sid = sessionId || "claude";
  const pacer = createSnapshotPacer({
    emit: (text) => emitClaudeReasoningTimeline(text, "running", entry.sessionId, blockKey),
  });
  entry = { pacer, lastText: "", sessionId: sid };
  ctx.reasoningBlocks.set(blockKey, entry);
  return entry;
}

/**
 * 块结束（content_block_stop）：丢掉 pacer 还没吐的 running 帧，直接发一条终态
 * snapshot（status='success'）。UI 标题从"正在思考"切到"已思考"，且 success
 * 是终态会绕过 Rust throttle 立即落库。
 */
function closeClaudeReasoningBlock(ctx, blockKey, sessionId) {
  const entry = ctx.reasoningBlocks.get(blockKey);
  if (!entry) return;
  entry.pacer.cancel();
  emitClaudeReasoningTimeline(
    entry.lastText,
    "success",
    sessionId || entry.sessionId,
    blockKey,
  );
  ctx.reasoningBlocks.delete(blockKey);
}

/**
 * Turn 结束时收尾所有还挂着的 reasoning block。正常路径下 content_block_stop
 * 已经把每块翻成 success；走到这里多半是 turn 中断 / SDK 没发 stop 的边界场景。
 * 顺便清理 dispatcher state 里的空 thinking block（content_block_start 没跟
 * 任何 delta 就 turn 结束），避免下个 turn 复用 index 时状态错乱。
 */
function finalizeClaudeReasoningTimeline(ctx, sessionId) {
  for (const [blockKey, entry] of ctx.reasoningBlocks) {
    entry.pacer.cancel();
    emitClaudeReasoningTimeline(
      entry.lastText,
      "success",
      sessionId || entry.sessionId,
      blockKey,
    );
  }
  ctx.reasoningBlocks.clear();
  // dispatcher 残留 block（空 thinking）的 state 清理；空内容 UI 会自然过滤掉。
  finalizeClaudeReasoningBlocks(ctx.claudeStream, () => {});
}

function normalizeAskUserResult(value) {
  if (!isRecord(value)) return { answers: {}, cancelled: true };
  return {
    answers: isRecord(value.answers) ? value.answers : {},
    cancelled: value.cancelled === true,
  };
}

function stableOptionId(index) {
  return `o-${index + 1}`;
}

function normalizeClaudeAskUserQuestions(input) {
  const rawQuestions = Array.isArray(input?.questions) ? input.questions.slice(0, 4) : [];
  return rawQuestions
    .map((question, questionIndex) => {
      if (!isRecord(question)) return null;
      const options = Array.isArray(question.options) ? question.options.slice(0, 6) : [];
      const normalizedOptions = options
        .map((option, optionIndex) => {
          if (!isRecord(option)) return null;
          const label = stringOrNull(option.label) || `Option ${optionIndex + 1}`;
          const normalized = {
            id: stableOptionId(optionIndex),
            label,
          };
          const description = stringOrNull(option.description);
          const preview = stringOrNull(option.preview);
          if (description) normalized.description = description;
          if (preview) normalized.preview = preview;
          if (optionIndex === 0) normalized.recommended = true;
          return normalized;
        })
        .filter(Boolean);
      if (normalizedOptions.length < 2) return null;
      return {
        id: `q-${questionIndex + 1}`,
        header: shortText(question.header, 12) || `问题 ${questionIndex + 1}`,
        question: shortText(question.question, 1200) || "请选择一个选项。",
        mode: question.multiSelect === true ? "multi" : "single",
        options: normalizedOptions,
        allowOther: true,
        skippable: false,
      };
    })
    .filter(Boolean);
}

function claudeAskUserInputToSpec(input) {
  return {
    title: "Claude 想确认一下",
    source: "Claude",
    dismissable: true,
    questions: normalizeClaudeAskUserQuestions(input),
  };
}

function answerValueToLabels(answer, question, originalQuestion) {
  if (!answer) return null;
  if (answer.skipped) return null;
  const sourceOptions = Array.isArray(originalQuestion?.options) ? originalQuestion.options : [];
  const labels = new Map(
    question.options.map((option, index) => [
      option.id,
      stringOrNull(sourceOptions[index]?.label) || option.label,
    ]),
  );
  const one = (value) => {
    if (value === "other") return stringOrNull(answer.notes) || "Other";
    return labels.get(value) || stringOrNull(value);
  };
  if (Array.isArray(answer.value)) {
    return answer.value.map(one).filter(Boolean).join(", ");
  }
  return one(answer.value);
}

function askUserResultToClaudeOutput(input, spec, result) {
  const originalQuestions = Array.isArray(input?.questions) ? input.questions : [];
  const answers = {};
  const annotations = {};
  for (let index = 0; index < spec.questions.length; index += 1) {
    const question = spec.questions[index];
    const originalQuestion = originalQuestions[index];
    const questionText =
      stringOrNull(originalQuestion?.question) ||
      question.question ||
      `Question ${index + 1}`;
    const answer = result.answers?.[question.id];
    const labelText = answerValueToLabels(answer, question, originalQuestion);
    if (labelText) answers[questionText] = labelText;
    if (answer?.notes) annotations[questionText] = { notes: answer.notes };
  }
  return {
    questions: originalQuestions.slice(0, spec.questions.length),
    answers,
    annotations,
    cancelled: result.cancelled === true,
  };
}

async function handleClaudeAskUserQuestion(input) {
  const spec = claudeAskUserInputToSpec(input);
  if (spec.questions.length === 0) {
    return {
      content: [{ type: "text", text: "No valid questions were provided." }],
      isError: true,
    };
  }
  const result = await requestAskUser(spec);
  const output = askUserResultToClaudeOutput(input, spec, result);
  return {
    content: [{ type: "text", text: JSON.stringify(output) }],
    structuredContent: output,
    isError: output.cancelled,
  };
}

const askUserQuestionInputSchema = {
  questions: z
    .array(
      z.object({
        question: z.string(),
        header: z.string(),
        options: z
          .array(
            z.object({
              label: z.string(),
              description: z.string().optional().default(""),
              preview: z.string().optional(),
            }),
          )
          .min(2)
          .max(4),
        multiSelect: z.boolean().optional().default(false),
      }),
    )
    .min(1)
    .max(4),
};

const liliaAskUserServer = createSdkMcpServer({
  name: "lilia",
  version: "0.1.0",
  tools: [
    tool(
      "ask_user_question",
      "Ask the human user one or more multiple-choice questions through Lilia.",
      askUserQuestionInputSchema,
      handleClaudeAskUserQuestion,
      { alwaysLoad: true },
    ),
  ],
  alwaysLoad: true,
});

/**
 * Claude 工具调用 → lilia 协议事件：通过 `@lilia/contracts/claudeTools.mjs` 的
 * `normalizeClaudeTool()` 适配表把 (toolName, input) 折成 `{kind, subkind, payload}`，
 * runner 直接按 lilia 协议 emit timeline event。
 *
 * runner 不知道任何 display / 渲染细节 —— display 由前端
 * `deriveTimelineDisplay()` 按 lilia 协议（liliaTools.mjs）现算。
 */
function emitClaudeToolTimeline(block, msg, ctx) {
  const name = stringOrNull(block?.name) || "tool";
  const input = isRecord(block?.input) ? block.input : {};
  const sourceId = stringOrNull(block?.id || block?.tool_use_id || msg?.uuid);
  const normalized = normalizeClaudeTool(name, input, {
    subagent_type: msg?.subagent_type,
    task_description: msg?.task_description,
  });
  const payload = {
    backend: "claude",
    toolName: name,
    ...normalized.payload,
    sessionId: msg?.session_id,
  };
  if (normalized.subkind) payload.subkind = normalized.subkind;
  if (ctx?.activeTools && sourceId) {
    ctx.activeTools.set(sourceId, { name, kind: normalized.kind, subkind: normalized.subkind, payload: normalized.payload });
  }
  emitTimeline({
    kind: normalized.kind,
    status: "started",
    title: name,
    summary: normalized.summary,
    payload,
    sourceId,
  });
}

/**
 * Claude SDK 把工具结果放在「user」消息的 tool_result 块里，自带
 * `tool_use_id` 关联到原 tool_use。这里按相同 sourceId upsert 一条终态事件，
 * 让前端时间线节点从 running 翻成 success/error，不再卡在「跑中」icon 上。
 */
function emitClaudeToolResultTimeline(block, msg, ctx) {
  const sourceId = stringOrNull(block?.tool_use_id);
  if (!sourceId) return;
  const cached = ctx?.activeTools?.get(sourceId);
  const name = cached?.name || "tool";
  const cachedPayload = cached?.payload || {};
  const kind = cached?.kind || "tool";
  const subkind = cached?.subkind || null;
  const isError = block?.is_error === true;
  let text = "";
  if (typeof block?.content === "string") {
    text = block.content;
  } else if (Array.isArray(block?.content)) {
    text = block.content
      .filter((c) => c && c.type === "text" && typeof c.text === "string")
      .map((c) => c.text)
      .join("\n");
  }
  // 工具完成时不要把 output 写进 summary —— 折叠预览由 deriveTimelineDisplay 按
  // lilia 协议从 payload 派生（command → command 文本，file_read → path…），输出
  // 用 payload.output 留给展开态的 OUTPUT 代码块。错误信息仍保留进 summary，
  // 折叠预览能直接看到失败原因。
  const summary = isError ? shortText(text, 400) || "" : "";
  const payload = {
    backend: "claude",
    toolName: name,
    ...cachedPayload,
    isError,
    output: text,
    sessionId: msg?.session_id,
  };
  if (subkind) payload.subkind = subkind;
  emitTimeline({
    kind,
    status: isError ? "error" : "success",
    title: name,
    summary,
    payload,
    sourceId,
  });
  ctx?.activeTools?.delete(sourceId);
}

/**
 * Turn 结束前的兜底：把任何还没收到 tool_result 的工具一次性翻成终态，
 * 否则前端时间线会永远停在「跑中」状态。
 */
function sweepActiveClaudeTools(ctx, status, sessionId) {
  if (!ctx?.activeTools || ctx.activeTools.size === 0) return;
  for (const [sourceId, info] of ctx.activeTools) {
    const payload = {
      backend: "claude",
      toolName: info.name,
      ...(info.payload || {}),
      sweptByTurnEnd: true,
      sessionId,
    };
    if (info.subkind) payload.subkind = info.subkind;
    emitTimeline({
      kind: info.kind,
      status,
      title: info.name,
      summary: "",
      payload,
      sourceId,
    });
  }
  ctx.activeTools.clear();
}

function mapClaudeSystemTimeline(msg) {
  if (!isRecord(msg)) return;
  const subtype = stringOrNull(msg.subtype) || "";

  if (msg.type === "tool_progress") {
    const name = msg.tool_name || "tool";
    const normalized = normalizeClaudeTool(name, {});
    const payload = {
      backend: "claude",
      toolName: name,
      elapsedTimeSeconds: msg.elapsed_time_seconds,
      sessionId: msg.session_id,
    };
    if (normalized.subkind) payload.subkind = normalized.subkind;
    emitTimeline({
      kind: normalized.kind,
      status: "running",
      title: name,
      summary: `${msg.elapsed_time_seconds ?? 0}s`,
      payload,
      sourceId: msg.tool_use_id || msg.uuid,
    });
    return;
  }

  if (msg.type === "tool_use_summary") {
    emitTimeline({
      kind: "tool",
      status: "success",
      title: "Tool summary",
      summary: msg.summary,
      payload: {
        backend: "claude",
        precedingToolUseIds: msg.preceding_tool_use_ids,
        sessionId: msg.session_id,
      },
      sourceId: msg.uuid,
    });
    return;
  }

  if (msg.type === "auth_status") {
    const text = Array.isArray(msg.output) ? msg.output.join("\n") : msg.error;
    emitTimeline({
      kind: msg.error ? "error" : "turn",
      status: msg.error ? "error" : msg.isAuthenticating ? "running" : "success",
      title: "Authentication",
      summary: msg.error || text || "",
      payload: {
        backend: "claude",
        isAuthenticating: msg.isAuthenticating,
        sessionId: msg.session_id,
      },
      sourceId: msg.uuid,
    });
    return;
  }

  if (msg.type === "system") {
    switch (subtype) {
      case "init":
        emitTimeline({
          kind: "turn",
          status: "started",
          title: "Claude session",
          summary: msg.model || "",
          payload: {
            backend: "claude",
            model: msg.model,
            cwd: msg.cwd,
            permissionMode: msg.permissionMode,
            tools: msg.tools,
            mcpServers: msg.mcp_servers,
          },
          sourceId: msg.uuid,
        });
        return;
      case "task_started":
        emitTimeline({
          kind: "subagent",
          status: "started",
          title: msg.subagent_type || msg.task_type || "Task",
          summary: msg.description || msg.prompt || "",
          payload: {
            backend: "claude",
            description: msg.description,
            subagentType: msg.subagent_type,
            taskType: msg.task_type,
            workflowName: msg.workflow_name,
            sessionId: msg.session_id,
          },
          sourceId: msg.tool_use_id || msg.uuid,
        });
        return;
      case "task_progress":
        emitTimeline({
          kind: "subagent",
          status: "running",
          title: msg.subagent_type || "Task progress",
          summary: msg.summary || msg.description || msg.last_tool_name || "",
          payload: {
            backend: "claude",
            description: msg.description,
            subagentType: msg.subagent_type,
            usage: msg.usage,
            lastToolName: msg.last_tool_name,
            sessionId: msg.session_id,
          },
          sourceId: msg.tool_use_id || msg.uuid,
        });
        return;
      case "task_updated": {
        const patch = isRecord(msg.patch) ? msg.patch : {};
        emitTimeline({
          kind: "subagent",
          status: normalizeTimelineStatus(patch.status || "running"),
          title: "Task updated",
          summary: patch.error || patch.description || "",
          payload: {
            backend: "claude",
            patch,
            sessionId: msg.session_id,
          },
          sourceId: msg.uuid,
        });
        return;
      }
      case "task_notification":
        emitTimeline({
          kind: msg.status === "failed" ? "error" : "subagent",
          status: normalizeTimelineStatus(msg.status || "success"),
          title: "Task notification",
          summary: msg.summary || "",
          payload: {
            backend: "claude",
            status: msg.status,
            outputFile: msg.output_file,
            usage: msg.usage,
            sessionId: msg.session_id,
          },
          sourceId: msg.tool_use_id || msg.uuid,
        });
        return;
      case "notification":
        emitTimeline({
          kind: "turn",
          status: msg.priority === "immediate" ? "requires_action" : "info",
          title: msg.key || "Notification",
          summary: msg.text || "",
          payload: {
            backend: "claude",
            priority: msg.priority,
            color: msg.color,
            timeoutMs: msg.timeout_ms,
            sessionId: msg.session_id,
          },
          sourceId: msg.uuid,
        });
        return;
      case "api_retry":
        emitTimeline({
          kind: "turn",
          status: "running",
          title: "API retry",
          summary: msg.error || "",
          payload: {
            backend: "claude",
            attempt: msg.attempt,
            maxRetries: msg.max_retries,
            retryDelayMs: msg.retry_delay_ms,
            errorStatus: msg.error_status,
            sessionId: msg.session_id,
          },
          sourceId: msg.uuid,
        });
        return;
      case "status":
        emitTimeline({
          kind: "turn",
          status: msg.status || msg.compact_result || "info",
          title: "Claude status",
          summary: msg.compact_error || msg.status || "",
          payload: {
            backend: "claude",
            status: msg.status,
            permissionMode: msg.permissionMode,
            compactResult: msg.compact_result,
            sessionId: msg.session_id,
          },
          sourceId: msg.uuid,
        });
        return;
      case "session_state_changed":
        emitTimeline({
          kind: "turn",
          status: msg.state || "info",
          title: "Session state",
          summary: msg.state || "",
          payload: {
            backend: "claude",
            state: msg.state,
            sessionId: msg.session_id,
          },
          sourceId: msg.uuid,
        });
        return;
      case "hook_started":
      case "hook_progress":
      case "hook_response":
        emitTimeline({
          kind: "tool",
          status:
            subtype === "hook_started"
              ? "started"
              : msg.outcome === "error"
                ? "error"
                : normalizeTimelineStatus(msg.outcome || "running"),
          title: msg.hook_name || "Hook",
          summary: msg.output || msg.stderr || msg.stdout || msg.hook_event || "",
          payload: {
            backend: "claude",
            hookName: msg.hook_name,
            hookEvent: msg.hook_event,
            exitCode: msg.exit_code,
            sessionId: msg.session_id,
          },
          sourceId: msg.uuid,
        });
        return;
      case "permission_denied":
        emitTimeline({
          kind: "error",
          status: "error",
          title: msg.tool_name || "Permission denied",
          summary: msg.message || msg.decision_reason || "",
          payload: {
            backend: "claude",
            toolName: msg.tool_name,
            decisionReasonType: msg.decision_reason_type,
            decisionReason: msg.decision_reason,
            sessionId: msg.session_id,
          },
          sourceId: msg.tool_use_id || msg.uuid,
        });
        return;
      case "mirror_error":
        emitTimeline({
          kind: "error",
          status: "error",
          title: "Mirror error",
          summary: msg.error || "",
          payload: {
            backend: "claude",
            key: msg.key,
            sessionId: msg.session_id,
          },
          sourceId: msg.uuid,
        });
        return;
      default:
        break;
    }
  }

  if (msg.error) {
    emitTimeline({
      kind: "error",
      status: "error",
      title: stringOrNull(msg.type) || "Claude error",
      summary: msg.error,
      payload: {
        backend: "claude",
        type: msg.type,
        subtype,
        sessionId: msg.session_id,
      },
      sourceId: msg.uuid,
    });
  }
}

async function runClaude(cmd) {
  const { cwd, prompt, model, resumeSessionId, permission } = cmd;
  const permOpts = mapClaudePermission(permission);
  const options = {
    cwd: cwd || process.cwd(),
    model: model || undefined,
    resume: resumeSessionId || undefined,
    includePartialMessages: true,
    // 注：full/readonly 也注册 canUseTool 是无副作用的 —— SDK 在 bypass/plan
    // 模式下不会触发回调；ask 模式下才会把请求转给我们。
    canUseTool: claudeCanUseTool,
    // SDK 1.0 默认不注入任何 system prompt——既丢了 Claude Code 内置的工具说明，
    // 也让模型不知道当前平台/shell，于是在 Windows 上偶尔会发 PowerShell 命令进
    // Bash 工具。启用 claude_code 预设拿回基础上下文，Windows 上 append 一段
    // shell 约束，见 buildClaudeSystemPrompt。
    systemPrompt: buildClaudeSystemPrompt(),
    mcpServers: {
      lilia: liliaAskUserServer,
    },
    toolAliases: {
      AskUserQuestion: "mcp__lilia__ask_user_question",
    },
    toolConfig: {
      askUserQuestion: { previewFormat: "markdown" },
    },
    ...permOpts,
    // SDK 默认会启用 Claude Code 的全套工具（Read/Write/Bash/...）。这正是
    // 「Lilia 是 Claude Code 的图形外壳」这一定位要的——不裁剪 tools。
  };

  let lastSessionId = null;
  const ctx = {
    claudeStream: createClaudeStreamState(),
    /** blockKey → { pacer, accumulatedText, sessionId }；每个 text block 一个独立
     *  pacer，sourceId 也按 blockKey 拆开。结束 turn 时统一翻成 success。 */
    textFragments: new Map(),
    /** blockKey → { pacer, lastText, sessionId }；每个 thinking block 一个独立
     *  snapshot pacer，按 33ms 节奏吐累计快照；block stop 时直接 emit success。 */
    reasoningBlocks: new Map(),
    /** 见过任何 type==="text" 的 assistant content block；用于 result 兜底判断是否
     *  该 emit 一张空 final 卡（vs 纯思考/纯 tool_use 的合法静默 turn）。 */
    sawAssistantTextBlock: false,
    resultSeen: false,
    /** sourceId → { kind, title, display }，给未收到 tool_result 的工具做收尾用。 */
    activeTools: new Map(),
  };
  for await (const msg of query({ prompt, options })) {
    if (msg.session_id) lastSessionId = msg.session_id;

    try {
      mapClaudeSystemTimeline(msg);
      switch (msg.type) {
        case "stream_event": {
          handleClaudeStreamEvent(msg, ctx);
          break;
        }
      case "assistant": {
        const content = msg?.message?.content;
        if (Array.isArray(content)) {
          for (const b of content) {
            if (b && b.type === "text") {
              // 仅记一个标志，不回填 pacer：SDK 把每个完成的 block 单发一条
              // assistant 消息（新 block 永远落 content[0]），按数组 index 当
              // sdkIndex 回填会跟 stream_event 的 blockKey 错位，产生 text:N /
              // text:N+1 两份重复。文本真正的内容由 stream_event 通道独占。
              ctx.sawAssistantTextBlock = true;
            }
            if (b && b.type === "tool_use") {
              emit({ type: "tool_use", name: b.name, input: b.input });
              emitClaudeToolTimeline(b, msg, ctx);
            }
          }
        }
        break;
      }
      case "result": {
        ctx.resultSeen = true;
        const errorSummary = msg.is_error
          ? (Array.isArray(msg.errors) ? msg.errors.join("\n") : msg.subtype) || ""
          : "";
        const status = msg.is_error ? "error" : "success";
        const fragmentsEmitted = finalizeClaudeTextFragments(ctx, status);
        sweepActiveClaudeTools(ctx, status, msg?.session_id);
        finalizeClaudeReasoningTimeline(ctx, msg?.session_id || lastSessionId);
        emitClaudeTextResultFallback(ctx, msg, status, fragmentsEmitted, lastSessionId);
        emitTimeline({
          kind: "turn",
          status,
          title: msg.is_error ? "Claude turn failed" : "Claude turn completed",
          summary: errorSummary,
          payload: {
            backend: "claude",
            subtype: msg.subtype,
            stopReason: msg.stop_reason,
            terminalReason: msg.terminal_reason,
            totalCostUsd: msg.total_cost_usd,
            usage: msg.usage,
            modelUsage: msg.modelUsage,
            permissionDenials: msg.permission_denials,
            errors: msg.errors,
            sessionId: msg.session_id || lastSessionId,
          },
          sourceId: msg.uuid,
        });
        emit({
          type: "done",
          sessionId: msg.session_id || lastSessionId,
          subtype: msg.subtype,
        });
        break;
      }
      case "user":
      case "user_replay": {
        // SDK 把 tool_result 放在 user/user_replay 消息的 content 里，
        // 用 tool_use_id 跟 assistant 那边的 tool_use 块关联。
        const content = msg?.message?.content;
        if (Array.isArray(content)) {
          for (const b of content) {
            if (b && b.type === "tool_result") {
              emitClaudeToolResultTimeline(b, msg, ctx);
            }
          }
        }
        break;
      }
      case "system":
      default:
        // system init 等已在 mapClaudeSystemTimeline 处理；其余暂忽略。
        break;
      }
    } catch (err) {
      emitTimeline({
        kind: "error",
        status: "error",
        title: "Claude event mapping",
        summary: err?.message || String(err),
        payload: {
          backend: "claude",
          rawType: msg?.type,
        },
        sourceId: msg?.uuid,
      });
    }
  }
  if (lastSessionId && !ctx.resultSeen) {
    // 兜底：万一某些 result 路径没有触发 done 事件
    const fragmentsEmitted = finalizeClaudeTextFragments(ctx, "success");
    sweepActiveClaudeTools(ctx, "success", lastSessionId);
    finalizeClaudeReasoningTimeline(ctx, lastSessionId);
    emitClaudeTextResultFallback(ctx, null, "success", fragmentsEmitted, lastSessionId);
    emitTimeline({
      kind: "turn",
      status: "success",
      title: "Claude turn completed",
      summary: "",
      payload: {
        backend: "claude",
        subtype: "success",
        sessionId: lastSessionId,
      },
      sourceId: `${lastSessionId}:turn:done`,
    });
    emit({ type: "done", sessionId: lastSessionId, subtype: "success" });
  }
}

/** 把 stream_event 分到 per-block text / reasoning pacer。 */
function handleClaudeStreamEvent(msg, ctx) {
  dispatchClaudeStreamEvent({
    event: msg?.event,
    state: ctx.claudeStream,
    onTextStart: ({ blockKey }) => {
      if (blockKey === null || blockKey === undefined) return;
      // 提前占住 timeline order：pacer 首次 tick 有 33ms 延迟，光等
      // onTextDelta 会让短开场白被同 turn 的 tool_use 抢到更小的 order。
      // 这里同步 emit 一条空 running 卡片，sourceId 立刻注册；后续 pacer 走
      // 同 sourceId 的 upsert 把内容填进去，DB 行的 order 不会再变。
      getOrCreateClaudeTextFragment(ctx, blockKey, msg?.session_id);
      emitAssistantTextFragmentTimeline("", "running", msg?.session_id, blockKey);
    },
    onTextDelta: ({ blockKey, text }) => {
      if (blockKey === null || blockKey === undefined) return;
      const fragment = getOrCreateClaudeTextFragment(ctx, blockKey, msg?.session_id);
      fragment.accumulatedText += text;
      fragment.sessionId = msg?.session_id || fragment.sessionId;
      fragment.pacer.push(text);
    },
    onReasoning: ({ blockKey, text }) => {
      if (blockKey === null || blockKey === undefined) return;
      const entry = getOrCreateClaudeReasoningBlock(ctx, blockKey, msg?.session_id);
      entry.sessionId = msg?.session_id || entry.sessionId;
      entry.lastText = text;
      entry.pacer.push(text);
    },
    onReasoningClose: ({ blockKey }) => {
      if (blockKey === null || blockKey === undefined) return;
      closeClaudeReasoningBlock(ctx, blockKey, msg?.session_id);
    },
  });
}

function getOrCreateClaudeTextFragment(ctx, blockKey, sessionId) {
  let fragment = ctx.textFragments.get(blockKey);
  if (fragment) return fragment;
  const sid = sessionId || "claude";
  const pacer = createTextPacer({
    emit: (text) => emitAssistantTextFragmentTimeline(text, "running", fragment.sessionId, blockKey),
  });
  fragment = { pacer, accumulatedText: "", sessionId: sid };
  ctx.textFragments.set(blockKey, fragment);
  return fragment;
}

function finalizeClaudeTextFragments(ctx, status) {
  let emitted = 0;
  for (const [blockKey, fragment] of ctx.textFragments) {
    fragment.pacer.finishImmediate();
    emitAssistantTextFragmentTimeline(
      fragment.accumulatedText,
      status,
      fragment.sessionId,
      blockKey,
    );
    emitted += 1;
  }
  ctx.textFragments.clear();
  return emitted;
}

/**
 * 兜底：流式 fragment 全为空时，用 msg.result 落最终回复；连 result 都没有
 * 但本 turn 又见过 text block，就 emit 一条空卡，避免 UI 静默吞掉。
 */
function emitClaudeTextResultFallback(ctx, msg, status, fragmentsEmitted, lastSessionId) {
  if (fragmentsEmitted > 0) return;
  const resultText = fullTextOrNull(msg?.result);
  const sessionId = msg?.session_id || lastSessionId || "claude";
  if (resultText) {
    emitAssistantTextFragmentTimeline(resultText, status, sessionId, "result");
  } else if (ctx.sawAssistantTextBlock) {
    emitAssistantTextFragmentTimeline("", status, sessionId, "result");
  }
}

// ---------- Codex ----------

function mapCodexPermission(p) {
  // Lilia 三档 → Codex SDK 的 sandboxMode（0.47 起：字段从 sandbox 改名为
  // sandboxMode，approvalMode 已从 ThreadOptions 移除，交由 codex CLI 自身策略）。
  switch (p) {
    case "full":
      return { sandboxMode: "danger-full-access" };
    case "readonly":
      return { sandboxMode: "read-only" };
    case "ask":
    default:
      return { sandboxMode: "workspace-write" };
  }
}

function getCodexItemType(item) {
  return stringOrNull(item?.type || item?.item_type) || "";
}

function getCodexStatus(eventType, item) {
  const status = stringOrNull(item?.status);
  if (status) return normalizeTimelineStatus(status);
  if (eventType === "item.started") return "started";
  if (eventType === "item.updated") return "running";
  if (eventType === "item.completed") return "success";
  if (eventType === "turn.started") return "started";
  if (eventType === "turn.completed") return "success";
  if (eventType === "turn.failed" || eventType === "error") return "error";
  return "info";
}

function codexTimelineKindForItem(item) {
  switch (getCodexItemType(item)) {
    case "reasoning":
      return { kind: "reasoning" };
    case "command_execution":
      return { kind: "command" };
    case "file_change":
      return { kind: "file_change" };
    case "mcp_tool_call":
      return { kind: "mcp" };
    case "web_search":
      return { kind: "search", subkind: "web" };
    case "todo_list":
      return { kind: "todo_list" };
    case "error":
      return { kind: "error" };
    default:
      return null;
  }
}

function summarizeCodexTodoList(items) {
  if (!Array.isArray(items)) return "";
  return items
    .map((todo) => {
      if (!isRecord(todo)) return shortText(todo, 120);
      const prefix = todo.completed ? "[x]" : "[ ]";
      return `${prefix} ${shortText(todo.text, 160) || ""}`.trim();
    })
    .filter(Boolean)
    .join("\n");
}

function summarizeCodexFileChanges(changes) {
  if (!Array.isArray(changes)) return "";
  return changes
    .map((change) => {
      if (!isRecord(change)) return shortText(change, 160);
      const path = shortText(change.path, 240) || "(unknown path)";
      return `${change.kind || "update"} ${path}`;
    })
    .filter(Boolean)
    .join("\n");
}

function codexTimelineTitle(kind, item, eventType) {
  switch (kind) {
    case "reasoning":
      return "Reasoning";
    case "command":
      return shortText(item.command, 200) || "Command";
    case "file_change":
      return "File change";
    case "mcp":
      return [item.server, item.tool].filter(Boolean).join(" / ") || "MCP tool";
    case "search":
      return shortText(item.query, 200) || "Web search";
    case "todo_list":
      return eventType === "item.completed" ? "Plan completed" : "Plan";
    case "error":
      return "Error";
    default:
      return kind;
  }
}

function codexTimelineSummary(kind, item) {
  switch (kind) {
    case "reasoning":
      return shortText(item.text, 1200) || "";
    case "command":
      // 折叠预览要保持稳定地显示指令本身——派生器从 payload.command 算出 object，
      // 这里把 command 当作 summary 兜底，避免 item.aggregated_output 在命令结束后
      // 反过来把指令覆盖成输出。
      return shortText(item.command, 1200) || "";
    case "file_change":
      return summarizeCodexFileChanges(item.changes);
    case "mcp":
      return [item.server, item.tool].filter(Boolean).join(" / ");
    case "search":
      return shortText(item.query, 1200) || "";
    case "todo_list":
      return summarizeCodexTodoList(item.items);
    case "error":
      return shortText(item.message, 1200) || "";
    default:
      return "";
  }
}

function codexTimelinePayload(kind, subkind, item, eventType) {
  const base = {
    backend: "codex",
    eventType,
    itemType: getCodexItemType(item),
    status: item?.status,
  };
  if (subkind) base.subkind = subkind;
  switch (kind) {
    case "reasoning":
      return { ...base, text: item.text };
    case "command":
      return {
        ...base,
        command: item.command,
        output: item.aggregated_output,
        exit: item.exit_code,
      };
    case "file_change":
      return { ...base, changes: item.changes };
    case "mcp":
      return { ...base, server: item.server, tool: item.tool };
    case "search":
      return { ...base, query: item.query };
    case "todo_list":
      return { ...base, items: item.items };
    case "error":
      return { ...base, message: item.message };
    default:
      return base;
  }
}

function emitCodexItemTimeline(eventType, item) {
  const route = codexTimelineKindForItem(item);
  if (!route) return;
  const { kind, subkind = null } = route;
  emitTimeline({
    kind,
    status: getCodexStatus(eventType, item),
    title: codexTimelineTitle(kind, item, eventType),
    summary: codexTimelineSummary(kind, item),
    payload: codexTimelinePayload(kind, subkind, item, eventType),
    sourceId: item?.id,
  });
}

function emitCodexTurnTimeline(eventType, ev, ctx) {
  const errorMessage = ev?.error?.message || ev?.message || "";
  emitTimeline({
    kind: "turn",
    status: getCodexStatus(eventType, null),
    title:
      eventType === "turn.started"
        ? "Codex turn started"
        : eventType === "turn.completed"
          ? "Codex turn completed"
          : "Codex turn failed",
    summary: errorMessage || "",
    payload: {
      backend: "codex",
      eventType,
      usage: ev?.usage,
      error: ev?.error,
      sessionId: ctx?.lastThreadId,
    },
  });
}

/**
 * 把 @openai/codex-sdk 的 ThreadEvent 翻译为 Lilia 的 NDJSON 协议。
 * SDK 的事件 schema 可能在 minor 版本间漂移，所有 picker 都做防御性 fallback。
 */
function mapCodexEventToNdjson(ev, ctx) {
  if (!ev || typeof ev !== "object") return;

  const tid = ev.thread_id || ev.threadId;
  if (tid && typeof tid === "string") ctx.lastThreadId = tid;

  const type = ev.type || "";

  if (type === "thread.started") return;

  if (type === "turn.started") {
    emitCodexTurnTimeline(type, ev, ctx);
    return;
  }

  // 文本增量：0.47 的 item.updated 给的是累积后的 ThreadItem.text（不是 delta），
  // 用 itemTextSeen 按 item.id 跟踪已发长度提取增量。若上游 SDK 改回发 delta
  // （total < seen），按 raw 直发兜底。
  if (type === "item.updated") {
    const item = ev.item || ev;
    const kind = item?.type || item?.item_type;
    emitCodexItemTimeline(type, item);
    if (
      (kind === "agent_message" || kind === "assistant_message") &&
      typeof item?.text === "string" &&
      item?.id
    ) {
      const seen = ctx.itemTextSeen.get(item.id) || 0;
      const total = item.text.length;
      if (total > seen) {
        const delta = item.text.slice(seen);
        ctx.assistantDeltaText += delta;
        ctx.assistantSnapshotText = item.text;
        ctx.pacer.push(delta);
        ctx.itemTextSeen.set(item.id, total);
      } else if (total < seen) {
        ctx.assistantDeltaText += item.text;
        ctx.assistantSnapshotText = item.text;
        ctx.pacer.syncTo(ctx.assistantSnapshotText);
      }
      return;
    }
    // 老 SDK / 未识别 item 类型走 picker 兜底。
    const text = pickCodexDeltaText(ev);
    if (text) {
      ctx.assistantDeltaText += text;
      ctx.pacer.push(text);
    }
    return;
  }

  // 完成的 item：agent_message 触发最终回复 timeline，其它当 tool_use。
  if (type === "item.completed" || type === "item.started") {
    const item = ev.item || ev;
    const kind = item?.item_type || item?.type;
    emitCodexItemTimeline(type, item);
    if (
      kind === "agent_message" ||
      kind === "assistant_message" ||
      kind === "message"
    ) {
      const text = pickCodexAssistantText(item);
      if (text && type === "item.completed") {
        ctx.assistantSnapshotText = text;
        ctx.pacer.finishImmediate();
        emitAssistantMessageTimeline(text, "success", "codex");
        if (item?.id) ctx.itemTextSeen.delete(item.id);
      }
      return;
    }
    if (type === "item.started") {
      const name = String(kind || "tool");
      const { item_type: _ignore, type: _ignore2, ...rest } = item || {};
      emit({ type: "tool_use", name, input: rest });
    }
    return;
  }

  if (type === "turn.completed") {
    ctx.turnCompletedSeen = true;
    ctx.pacer.finishImmediate();
    emitCodexTurnTimeline(type, ev, ctx);
    emit({
      type: "done",
      sessionId: ctx.lastThreadId,
      subtype: "success",
    });
    return;
  }

  if (type === "turn.failed" || type === "error") {
    const msg = ev.error?.message || ev.message || "codex turn failed";
    ctx.pacer.finishImmediate();
    emitCodexTurnTimeline(type, ev, ctx);
    emit({ type: "error", message: msg });
  }
}

function pickCodexDeltaText(ev) {
  if (typeof ev.delta === "string") return ev.delta;
  if (typeof ev.text === "string") return ev.text;
  const item = ev.item;
  if (item && typeof item === "object") {
    if (typeof item.delta === "string") return item.delta;
    if (typeof item.text === "string") return item.text;
  }
  return null;
}

function pickCodexAssistantText(item) {
  if (!item) return "";
  if (typeof item.text === "string") return item.text;
  if (typeof item.content === "string") return item.content;
  if (Array.isArray(item.content)) {
    return item.content
      .filter((b) => b && (b.type === "text" || b.type === "output_text"))
      .map((b) => (typeof b.text === "string" ? b.text : ""))
      .join("");
  }
  return "";
}

async function runCodex(cmd) {
  const { cwd, prompt, model, resumeSessionId, permission } = cmd;

  // 动态 import：让仅用 Claude 的环境不需要装 @openai/codex-sdk 也能跑。
  let Codex;
  try {
    ({ Codex } = await import("@openai/codex-sdk"));
  } catch (err) {
    emit({
      type: "error",
      message:
        "未安装 @openai/codex-sdk，请在仓库根目录 `yarn install`，并确保已全局安装 codex CLI。",
    });
    process.exit(1);
    return;
  }

  const permOpts = mapCodexPermission(permission);
  const codex = new Codex({
    apiKey: process.env.OPENAI_API_KEY,
    baseUrl: process.env.OPENAI_BASE_URL || undefined,
  });

  const thread = resumeSessionId
    ? codex.resumeThread(resumeSessionId)
    : codex.startThread({
        workingDirectory: cwd || process.cwd(),
        model: model || undefined,
        ...permOpts,
      });

  const ctx = {
    lastThreadId: thread?.id ?? resumeSessionId ?? null,
    itemTextSeen: new Map(),
    assistantDeltaText: "",
    assistantSnapshotText: "",
    turnCompletedSeen: false,
    pacer: createTextPacer({
      emit: (text) => emitAssistantMessageTimeline(text, "running", "codex"),
    }),
  };

  // 0.47 起 thread.run() 返回完整 Turn（非流式），要拿事件流必须用 runStreamed。
  const turn = await thread.runStreamed(prompt);

  for await (const ev of turn.events) {
    mapCodexEventToNdjson(ev, ctx);
  }

  if (ctx.lastThreadId && !ctx.turnCompletedSeen) {
    // 兜底 done：与 Claude 分支语义一致——某些版本可能不发 turn.completed。
    const finalText =
      fullTextOrNull(ctx.assistantSnapshotText) ||
      fullTextOrNull(ctx.assistantDeltaText);
    ctx.pacer.finishImmediate();
    if (finalText) {
      emitAssistantMessageTimeline(finalText, "success", "codex");
    }
    emitTimeline({
      kind: "turn",
      status: "success",
      title: "Codex turn completed",
      summary: "",
      payload: {
        backend: "codex",
        eventType: "turn.completed",
        sessionId: ctx.lastThreadId,
      },
    });
    emit({ type: "done", sessionId: ctx.lastThreadId, subtype: "success" });
  }
}

// ---------- Dry run（单测用） ----------

async function runDryRun(cmd) {
  const backend = cmd.backend === "codex" ? "codex" : "claude";
  const sid = `dry-${backend}-xxx`;
  emitTimeline({
    kind: "turn",
    status: "started",
    title: `${backend} turn started`,
    summary: "Dry-run agent turn",
    payload: { backend, sessionId: sid },
    sourceId: `${sid}:turn:start`,
  });
  emitTimeline({
    kind: "reasoning",
    status: "running",
    title: "公开思考摘要",
    summary: "正在检查当前实现并规划下一步。",
    payload: { backend, source: "dry-run" },
    sourceId: `${sid}:reasoning`,
  });
  emitTimeline({
    kind: "command",
    status: "success",
    title: "yarn verify:contracts",
    summary: "Contracts verification completed.",
    payload: {
      backend,
      command: "yarn verify:contracts",
      exitCode: 0,
      aggregatedOutput: "ok",
    },
    sourceId: `${sid}:command`,
  });
  emitTimeline({
    kind: "file_change",
    status: "success",
    title: "File changes",
    summary: "update apps/desktop/src/components/chat/AgentTimeline.vue",
    payload: {
      backend,
      changes: [
        {
          kind: "update",
          path: "apps/desktop/src/components/chat/AgentTimeline.vue",
        },
      ],
    },
    sourceId: `${sid}:file-change`,
  });
  emitTimeline({
    kind: "subagent",
    status: "success",
    title: "Worker summary",
    summary: "子代理完成 timeline 样式切片。",
    payload: { backend, agentType: "worker" },
    sourceId: `${sid}:subagent`,
  });
  emitTimeline({
    kind: "todo_list",
    status: "success",
    title: "计划",
    summary: "[x] 接线 runner\n[x] 渲染 timeline",
    payload: {
      backend,
      items: [
        { text: "接线 runner", completed: true },
        { text: "渲染 timeline", completed: true },
      ],
    },
    sourceId: `${sid}:todo`,
  });
  emitTimeline({
    kind: "error",
    status: "error",
    title: "示例错误",
    summary: "Dry-run error sample for UI coverage.",
    payload: { backend, message: "Dry-run error sample" },
    sourceId: `${sid}:error`,
  });
  emitAssistantMessageTimeline("hello ", "running", backend);
  emitAssistantMessageTimeline("hello from ", "running", backend);
  emitAssistantMessageTimeline(`hello from ${backend}`, "running", backend);
  emitAssistantMessageTimeline(`hello from ${backend}`, "success", backend);
  emitTimeline({
    kind: "turn",
    status: "success",
    title: `${backend} turn completed`,
    summary: "",
    payload: {
      backend,
      sessionId: sid,
    },
    sourceId: `${sid}:turn:done`,
  });
  emit({ type: "done", sessionId: sid, subtype: "success" });
}

// ---------- 入口 ----------

async function main() {
  // 1) readline 拆 NDJSON 行：第一行是初始命令；后续行交给 handleControlLine
  //    （目前仅消费 consent_response）。父进程不再用 close-stdin 表示完事，
  //    runner 在 SDK 跑完后主动 exit。
  const rl = createInterface({ input: process.stdin });
  let cmd = null;
  let cmdResolve;
  const cmdReady = new Promise((resolve) => {
    cmdResolve = resolve;
  });
  rl.on("line", (line) => {
    if (!cmd) {
      try {
        cmd = JSON.parse(line);
      } catch (err) {
        emit({ type: "error", message: `invalid stdin JSON: ${err.message}` });
        process.exit(1);
      }
      cmdResolve(cmd);
      return;
    }
    handleControlLine(line);
  });

  cmd = await cmdReady;

  if (typeof cmd.prompt !== "string") {
    emit({ type: "error", message: "missing prompt" });
    process.exit(1);
  }
  cmd.prompt = buildPromptWithAttachments(cmd.prompt, cmd.attachments);
  if (cmd.prompt.trim().length === 0) {
    emit({ type: "error", message: "missing prompt" });
    process.exit(1);
  }

  if (process.env.LILIA_AGENT_DRY_RUN === "1") {
    try {
      await runDryRun(cmd);
    } catch (err) {
      emit({ type: "error", message: err?.message || String(err) });
      process.exit(1);
    }
    return;
  }

  const backend = cmd.backend === "codex" ? "codex" : "claude";
  try {
    if (backend === "codex") {
      await runCodex(cmd);
    } else {
      await runClaude(cmd);
    }
  } catch (err) {
    emit({ type: "error", message: err?.message || String(err) });
    process.exit(1);
  }
  // SDK 循环退出后主动收尾，不然 readline 会让进程挂在 stdin 上不退。
  rl.close();
  process.exit(0);
}

main().catch((err) => {
  emit({ type: "error", message: err?.message || String(err) });
  process.exit(1);
});
