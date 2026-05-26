// Lilia · Agent SDK 子进程包装器（双 backend：Claude / Codex）
//
// 调用约定：
//   - 父进程（Tauri Rust 端）启动 `node agent-runner.mjs`
//   - 父进程把一行 JSON 写到 stdin：
//       {
//         "backend": "claude" | "codex",
//         "cwd": "...",
//         "prompt": "...",
//         "model": "...",
//         "resumeSessionId": "...|null",
//         "permission": "full|ask|readonly"
//       }
//   - 父进程关闭 stdin
//   - 我们把 SDK 流出的事件按 NDJSON（一行一条）写到 stdout：
//       {"type":"chunk","text":"..."}              文本增量
//       {"type":"tool_use","name":"Read","input":{...}}
//       {"type":"assistant_done","text":"完整回复全文","sessionId":"..."}
//       {"type":"done","sessionId":"...","subtype":"success|error_..."}
//       {"type":"error","message":"..."}
//       {"type":"timeline","event":{"kind":"command","status":"success","title":"...","summary":"...","payload":{}}}
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

import { query } from "@anthropic-ai/claude-agent-sdk";
import { normalizeClaudeTool } from "@lilia/contracts/claudeTools.mjs";

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
 * Assistant 流式回复落到 message kind 的 timeline。
 * 用固定 sourceId="assistant"，Rust 端会拼成 `task:turn:assistant`，
 * 同一 turn 内反复 upsert 即可实现 token 级增量更新。
 */
function emitAssistantMessageTimeline(text, status, backend) {
  const content = typeof text === "string" ? text : "";
  emitTimeline({
    kind: "message",
    status,
    title: "Assistant",
    summary: content,
    payload: {
      role: "assistant",
      content,
      backend,
    },
    sourceId: "assistant",
  });
}

/**
 * "假流式" 派发器：上游 SDK 实测按 1-2s 节奏成批 flush stream_event，
 * 直接转发会让 UI 看到"块状"刷新。这里把到达的 chunk 推进 buffer，按 tick
 * 节奏（默认 30Hz、约每 6 tick 排空一批）从 buffer 取一段提交，UI 看到的
 * 就是平稳滚动的文本流。
 *
 * 终态调 `finishImmediate` 一次性提交剩余，上层紧接 emit success，保证
 * 最终态不被 pacer 延迟。`syncTo` 给 SDK 漏发 delta 但给完整 snapshot
 * （Claude case "assistant"）时补差量用。
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
  // - ask：默认行为，碰到敏感操作走 canUseTool（这里没接，等同 prompt 阻塞）。
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

/** 从 SDKPartialAssistantMessage.event 里抽出文本增量。 */
function extractClaudeTextDelta(streamEvent) {
  if (!streamEvent || typeof streamEvent !== "object") return null;
  if (streamEvent.type !== "content_block_delta") return null;
  const delta = streamEvent.delta;
  if (!delta || delta.type !== "text_delta") return null;
  return typeof delta.text === "string" ? delta.text : null;
}

function isClaudeThinkingSummaryContainer(value, fallbackType = "") {
  if (!isRecord(value)) return false;
  const type = String(value.type || fallbackType || "").toLowerCase();
  if (type.includes("redacted")) return false;
  return (
    type.includes("thinking") ||
    type.includes("reasoning") ||
    type.includes("summary") ||
    value.is_summary === true
  );
}

function rememberClaudeStreamBlock(streamEvent, ctx) {
  if (!isRecord(streamEvent) || !ctx?.streamBlocks) return;
  const index = streamEvent.index;
  if (index === undefined || index === null) return;

  if (streamEvent.type === "content_block_start") {
    const blockType = stringOrNull(streamEvent.content_block?.type);
    if (blockType) ctx.streamBlocks.set(index, blockType);
    return;
  }

  if (streamEvent.type === "content_block_stop") {
    ctx.streamBlocks.delete(index);
  }
}

function extractPublicClaudeThinkingSummary(streamEvent, ctx) {
  if (!streamEvent || typeof streamEvent !== "object") return null;

  const delta = isRecord(streamEvent.delta) ? streamEvent.delta : null;
  const contentBlock = isRecord(streamEvent.content_block)
    ? streamEvent.content_block
    : null;
  const blockType =
    streamEvent.index === undefined || streamEvent.index === null
      ? ""
      : ctx?.streamBlocks?.get(streamEvent.index) || "";
  const candidates = [delta, contentBlock, streamEvent];

  for (const candidate of candidates) {
    if (!candidate) continue;
    if (isClaudeThinkingSummaryContainer(candidate, blockType)) {
      // 抽顺序：delta 的 `thinking` 字段（thinking_delta 的实际文本载体）
      // 优先于 summary/text，避免 thinking_delta 因没有 summary 字段而漏抽。
      const summary =
        candidate.thinking ||
        candidate.summary ||
        candidate.text ||
        candidate.content ||
        candidate.delta ||
        candidate.thinking_summary;
      const text = shortText(summary, 1200);
      if (text) return text;
    }

    const nestedSummary = candidate.summary || candidate.thinking_summary;
    if (isRecord(nestedSummary)) {
      const text = shortText(nestedSummary.text || nestedSummary.summary, 1200);
      if (text) return text;
    } else {
      const text = shortText(nestedSummary, 1200);
      if (text) return text;
    }
  }

  return null;
}

/**
 * 思考事件按 stream_event.index 聚合：同一 thinking 块内的 delta 文本拼接成
 * 一条 reasoning timeline 事件，sourceId 与 block 一一对应。这样 UI 看到的是
 * 一条平稳增长的"思考中"卡片，而不是每个 delta 一条噪点。
 */
function emitClaudeStreamTimeline(msg, ctx) {
  const event = msg?.event;
  rememberClaudeStreamBlock(event, ctx);
  const delta = extractPublicClaudeThinkingSummary(event, ctx);
  if (!delta) return;

  const index =
    event?.index === undefined || event?.index === null
      ? "default"
      : String(event.index);
  if (!ctx.thinkingByIndex) ctx.thinkingByIndex = new Map();
  const accumulated = (ctx.thinkingByIndex.get(index) || "") + delta;
  ctx.thinkingByIndex.set(index, accumulated);
  const sessionPrefix = msg?.session_id || "claude";
  const sourceId = `${sessionPrefix}:thinking:${index}`;

  emitTimeline({
    kind: "reasoning",
    status: "running",
    title: "思考中",
    summary: accumulated,
    payload: {
      backend: "claude",
      eventType: event?.type,
      deltaType: event?.delta?.type,
      blockType: event?.content_block?.type,
      sessionId: msg?.session_id,
      text: accumulated,
    },
    sourceId,
  });
}

/**
 * Turn 结束时把所有思考块标记成已完成。状态改成 success 后 UI 标题从
 * "正在思考" 变成 "已思考"，避免一直显示运行中。
 */
function finalizeClaudeThinkingBlocks(ctx, sessionId) {
  if (!ctx?.thinkingByIndex || ctx.thinkingByIndex.size === 0) return;
  for (const [index, text] of ctx.thinkingByIndex) {
    const sourceId = `${sessionId || "claude"}:thinking:${index}`;
    emitTimeline({
      kind: "reasoning",
      status: "success",
      title: "思考中",
      summary: text,
      payload: {
        backend: "claude",
        sessionId,
        text,
      },
      sourceId,
    });
  }
  ctx.thinkingByIndex.clear();
}

/** 从 SDKAssistantMessage.message.content 里抽出全部 text 块拼接结果。 */
function extractClaudeAssistantText(msg) {
  const content = msg?.message?.content;
  if (!Array.isArray(content)) return "";
  return content
    .filter((b) => b && b.type === "text" && typeof b.text === "string")
    .map((b) => b.text)
    .join("");
}

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
  const summary = shortText(text, 400) || "";
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
    ...permOpts,
    // SDK 默认会启用 Claude Code 的全套工具（Read/Write/Bash/...）。这正是
    // 「Lilia 是 Claude Code 的图形外壳」这一定位要的——不裁剪 tools。
  };

  let lastSessionId = null;
  const ctx = {
    streamBlocks: new Map(),
    assistantDeltaText: "",
    assistantSnapshotText: "",
    resultSeen: false,
    /** sourceId → { kind, title, display }，给未收到 tool_result 的工具做收尾用。 */
    activeTools: new Map(),
    /** block index → accumulated thinking text，turn 结束时统一翻成 success。 */
    thinkingByIndex: new Map(),
  };
  const pacer = createTextPacer({
    emit: (text) => emitAssistantMessageTimeline(text, "running", "claude"),
  });
  for await (const msg of query({ prompt, options })) {
    if (msg.session_id) lastSessionId = msg.session_id;

    try {
      mapClaudeSystemTimeline(msg);
      switch (msg.type) {
        case "stream_event": {
          emitClaudeStreamTimeline(msg, ctx);
          const text = extractClaudeTextDelta(msg.event);
          if (text) {
            ctx.assistantDeltaText += text;
            pacer.push(text);
          }
          break;
        }
      case "assistant": {
        // 含 tool_use 的 assistant 消息 text 是空串——跳过；纯文本块作为 delta 漏接时的兜底快照。
        const text = extractClaudeAssistantText(msg);
        if (text) {
          ctx.assistantSnapshotText = text;
          pacer.syncTo(text);
        }
        const content = msg?.message?.content;
        if (Array.isArray(content)) {
          for (const b of content) {
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
        const finalText =
          fullTextOrNull(msg.result) ||
          fullTextOrNull(ctx.assistantDeltaText) ||
          fullTextOrNull(ctx.assistantSnapshotText);
        const errorSummary = msg.is_error
          ? (Array.isArray(msg.errors) ? msg.errors.join("\n") : msg.subtype) || ""
          : "";
        pacer.finishImmediate();
        sweepActiveClaudeTools(ctx, msg.is_error ? "error" : "success", msg?.session_id);
        finalizeClaudeThinkingBlocks(ctx, msg?.session_id || lastSessionId);
        if (finalText) {
          emitAssistantMessageTimeline(
            finalText,
            msg.is_error ? "error" : "success",
            "claude",
          );
        }
        emitTimeline({
          kind: "turn",
          status: msg.is_error ? "error" : "success",
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
    const finalText =
      fullTextOrNull(ctx.assistantDeltaText) ||
      fullTextOrNull(ctx.assistantSnapshotText);
    pacer.finishImmediate();
    sweepActiveClaudeTools(ctx, "success", lastSessionId);
    finalizeClaudeThinkingBlocks(ctx, lastSessionId);
    if (finalText) {
      emitAssistantMessageTimeline(finalText, "success", "claude");
    }
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
      return (
        shortText(item.aggregated_output, 1200) ||
        shortText(item.command, 1200) ||
        ""
      );
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
  // 1) 读 stdin 上的 JSON 命令——只读一次直到 EOF。
  let raw = "";
  for await (const chunk of process.stdin) {
    raw += chunk;
  }
  let cmd;
  try {
    cmd = JSON.parse(raw);
  } catch (err) {
    emit({ type: "error", message: `invalid stdin JSON: ${err.message}` });
    process.exit(1);
  }

  const { prompt } = cmd;
  if (typeof prompt !== "string" || prompt.length === 0) {
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
}

main().catch((err) => {
  emit({ type: "error", message: err?.message || String(err) });
  process.exit(1);
});
