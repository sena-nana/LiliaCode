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
//   - 写完后进程 exit(0)；出错 exit(1)
//
// 不是为长连接设计——每次发送都起一个新进程。Node 启动 + SDK 加载 ~200ms 是
// 当前可以接受的延迟代价；后续可以换成持久子进程多路复用，但协议不变。
//
// 隐藏 env：LILIA_AGENT_DRY_RUN=1 时跳过真实 SDK，模拟一段 NDJSON——用于单测
// agent 调度链路而不消耗真实 API。

import { query } from "@anthropic-ai/claude-agent-sdk";

function emit(obj) {
  process.stdout.write(JSON.stringify(obj) + "\n");
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

/** 从 SDKAssistantMessage.message.content 里抽出全部 text 块拼接结果。 */
function extractClaudeAssistantText(msg) {
  const content = msg?.message?.content;
  if (!Array.isArray(content)) return "";
  return content
    .filter((b) => b && b.type === "text" && typeof b.text === "string")
    .map((b) => b.text)
    .join("");
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
  for await (const msg of query({ prompt, options })) {
    if (msg.session_id) lastSessionId = msg.session_id;

    switch (msg.type) {
      case "stream_event": {
        const text = extractClaudeTextDelta(msg.event);
        if (text) emit({ type: "chunk", text });
        break;
      }
      case "assistant": {
        // 完整文本块作为一次稳定快照（用于在 delta 漏接时兜底显示）。
        // 含 tool_use 的 assistant 消息这里 text 会是空串，跳过。
        const text = extractClaudeAssistantText(msg);
        if (text) {
          emit({ type: "assistant_done", text, sessionId: msg.session_id });
        }
        // 抽 tool_use 块单独发，给前端做「Claude 正在 Read X」提示用。
        const content = msg?.message?.content;
        if (Array.isArray(content)) {
          for (const b of content) {
            if (b && b.type === "tool_use") {
              emit({ type: "tool_use", name: b.name, input: b.input });
            }
          }
        }
        break;
      }
      case "result": {
        emit({
          type: "done",
          sessionId: msg.session_id || lastSessionId,
          subtype: msg.subtype,
        });
        break;
      }
      case "system":
      case "user":
      case "user_replay":
      default:
        // 第一阶段忽略，未来要可视化 tool_result / system init 时再开。
        break;
    }
  }
  if (lastSessionId) {
    // 兜底：万一某些 result 路径没有触发 done 事件
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

/**
 * 把 @openai/codex-sdk 的 ThreadEvent 翻译为 Lilia 的 NDJSON 协议。
 * SDK 的事件 schema 可能在 minor 版本间漂移，所有 picker 都做防御性 fallback。
 */
function mapCodexEventToNdjson(ev, ctx) {
  if (!ev || typeof ev !== "object") return;

  const tid = ev.thread_id || ev.threadId;
  if (tid && typeof tid === "string") ctx.lastThreadId = tid;

  const type = ev.type || "";

  if (type === "thread.started" || type === "turn.started") return;

  // 文本增量：0.47 的 item.updated 给的是累积后的 ThreadItem.text（不是 delta），
  // 用 itemTextSeen 按 item.id 跟踪已发长度提取增量。若上游 SDK 改回发 delta
  // （total < seen），按 raw 直发兜底。
  if (type === "item.updated") {
    const item = ev.item || ev;
    const kind = item?.type || item?.item_type;
    if (
      (kind === "agent_message" || kind === "assistant_message") &&
      typeof item?.text === "string" &&
      item?.id
    ) {
      const seen = ctx.itemTextSeen.get(item.id) || 0;
      const total = item.text.length;
      if (total > seen) {
        emit({ type: "chunk", text: item.text.slice(seen) });
        ctx.itemTextSeen.set(item.id, total);
      } else if (total < seen) {
        emit({ type: "chunk", text: item.text });
      }
      return;
    }
    // 老 SDK / 未识别 item 类型走 picker 兜底。
    const text = pickCodexDeltaText(ev);
    if (text) emit({ type: "chunk", text });
    return;
  }

  // 完成的 item：agent_message 走 assistant_done，其它当 tool_use。
  if (type === "item.completed" || type === "item.started") {
    const item = ev.item || ev;
    const kind = item?.item_type || item?.type;
    if (
      kind === "agent_message" ||
      kind === "assistant_message" ||
      kind === "message"
    ) {
      const text = pickCodexAssistantText(item);
      if (text && type === "item.completed") {
        emit({ type: "assistant_done", text, sessionId: ctx.lastThreadId });
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
    emit({
      type: "done",
      sessionId: ctx.lastThreadId,
      subtype: "success",
    });
    return;
  }

  if (type === "turn.failed" || type === "error") {
    const msg = ev.error?.message || ev.message || "codex turn failed";
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
  };

  // 0.47 起 thread.run() 返回完整 Turn（非流式），要拿事件流必须用 runStreamed。
  const turn = await thread.runStreamed(prompt);

  for await (const ev of turn.events) {
    mapCodexEventToNdjson(ev, ctx);
  }

  if (ctx.lastThreadId) {
    // 兜底 done：与 Claude 分支语义一致——某些版本可能不发 turn.completed。
    emit({ type: "done", sessionId: ctx.lastThreadId, subtype: "success" });
  }
}

// ---------- Dry run（单测用） ----------

async function runDryRun(cmd) {
  const backend = cmd.backend === "codex" ? "codex" : "claude";
  const sid = `dry-${backend}-xxx`;
  emit({ type: "chunk", text: "hello " });
  emit({ type: "chunk", text: "from " });
  emit({ type: "chunk", text: backend });
  emit({ type: "assistant_done", text: `hello from ${backend}`, sessionId: sid });
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
