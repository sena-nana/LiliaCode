// Lilia · Claude Agent SDK 子进程包装器
//
// 调用约定：
//   - 父进程（Tauri Rust 端）启动 `node agent-runner.mjs`
//   - 父进程把一行 JSON 写到 stdin：
//       {"cwd": "...", "prompt": "...", "model": "...", "resumeSessionId": "...|null", "permission": "full|ask|readonly"}
//   - 父进程关闭 stdin
//   - 我们把 SDK 流出的事件按 NDJSON（一行一条）写到 stdout：
//       {"type":"chunk","text":"..."}              文本增量（来自 stream_event.text_delta）
//       {"type":"tool_use","name":"Read","input":{...}}
//       {"type":"assistant_done","text":"完整回复全文","sessionId":"..."}
//       {"type":"done","sessionId":"...","subtype":"success|error_..."}
//       {"type":"error","message":"..."}
//   - 写完后进程 exit(0)；出错 exit(1)
//
// 不是为长连接设计——每次发送都起一个新进程。Node 启动 + SDK 加载 ~200ms 是
// 当前可以接受的延迟代价；后续可以换成持久子进程多路复用，但协议不变。

import { query } from "@anthropic-ai/claude-agent-sdk";

function emit(obj) {
  process.stdout.write(JSON.stringify(obj) + "\n");
}

function mapPermissionMode(p) {
  // Lilia 的三档语义 → SDK 的 PermissionMode。
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
function extractTextDelta(streamEvent) {
  if (!streamEvent || typeof streamEvent !== "object") return null;
  if (streamEvent.type !== "content_block_delta") return null;
  const delta = streamEvent.delta;
  if (!delta || delta.type !== "text_delta") return null;
  return typeof delta.text === "string" ? delta.text : null;
}

/** 从 SDKAssistantMessage.message.content 里抽出全部 text 块拼接结果。 */
function extractAssistantText(msg) {
  const content = msg?.message?.content;
  if (!Array.isArray(content)) return "";
  return content
    .filter((b) => b && b.type === "text" && typeof b.text === "string")
    .map((b) => b.text)
    .join("");
}

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

  const { cwd, prompt, model, resumeSessionId, permission } = cmd;
  if (typeof prompt !== "string" || prompt.length === 0) {
    emit({ type: "error", message: "missing prompt" });
    process.exit(1);
  }

  // 2) 调 SDK。失败时把异常映射成 error 事件再退出，避免 Rust 那边只看到非零
  //    退出码却没拿到原因。
  const permOpts = mapPermissionMode(permission);
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
  try {
    for await (const msg of query({ prompt, options })) {
      if (msg.session_id) lastSessionId = msg.session_id;

      switch (msg.type) {
        case "stream_event": {
          const text = extractTextDelta(msg.event);
          if (text) emit({ type: "chunk", text });
          break;
        }
        case "assistant": {
          // 完整文本块作为一次稳定快照（用于在 delta 漏接时兜底显示）。
          // 含 tool_use 的 assistant 消息这里 text 会是空串，跳过。
          const text = extractAssistantText(msg);
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
  } catch (err) {
    emit({ type: "error", message: err?.message || String(err) });
    process.exit(1);
  }
}

main().catch((err) => {
  emit({ type: "error", message: err?.message || String(err) });
  process.exit(1);
});
