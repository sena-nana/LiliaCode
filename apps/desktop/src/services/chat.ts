/**
 * Chat 服务层：把 Tauri command/event 包成 typed 函数，Vue 组件不直接碰
 * @tauri-apps/api。
 *
 * - 输入/输出形状全部走 @lilia/contracts，跨端共享。
 * - Rust 那侧用 `#[serde(rename_all = "camelCase")]`，所以这里不需要再做 key 映射。
 * - 流事件：assistant 的回复通过 chat:chunk 分片推回；done / error 用各自通道。
 *   组件订阅时把回调挂上去，离开页面时 await unlisten。
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ChatBranchOption,
  ChatComposerState,
  ChatMessage,
  ChatModelOption,
} from "@lilia/contracts";

export interface ChunkEvent { taskId: string; text: string; }
export interface ToolEvent { taskId: string; name: string; input: unknown; }
export interface DoneEvent { taskId: string; sessionId: string | null; subtype: string | null; }
export interface ErrorEvent { taskId: string; message: string; }

/** 当前对 Claude 的连接方式（与 Rust 端 ConnectionMode::as_str 对齐）。 */
export type ConnectionMode = "cc-switch" | "custom" | "direct" | "unconfigured";

export interface EnvStatus {
  hasApiKey: boolean;
  nodeAvailable: boolean;
  ccSwitchReachable: boolean;
  connectionMode: ConnectionMode;
  /** 实际请求会发往的 URL（兜底说明用），unconfigured 时为 null。 */
  effectiveUrl: string | null;
  /** 用户显式设的 ANTHROPIC_BASE_URL，没设为 null。 */
  envBaseUrl: string | null;
}

export function listMessages(taskId: string): Promise<ChatMessage[]> {
  return invoke<ChatMessage[]>("chat_list_messages", { taskId });
}

/**
 * 发起一轮对话。返回值是 user 那条消息本身（用于 reconcile 乐观渲染）；
 * assistant 的回复通过 onChunk/onDone/onError 事件异步推回。
 *
 * projectCwd 是 SDK 跑 agent loop 的工作目录——它决定 Claude 能看到的文件树。
 */
export function sendMessage(
  taskId: string,
  content: string,
  composer: ChatComposerState,
  projectCwd: string,
): Promise<ChatMessage> {
  return invoke<ChatMessage>("chat_send_message", {
    taskId,
    content,
    composer,
    projectCwd,
  });
}

export function listModels(): Promise<ChatModelOption[]> {
  return invoke<ChatModelOption[]>("chat_list_models");
}

export function listBranches(projectId: string): Promise<ChatBranchOption[]> {
  return invoke<ChatBranchOption[]>("chat_list_branches", { projectId });
}

export function getComposerState(taskId: string): Promise<ChatComposerState> {
  return invoke<ChatComposerState>("chat_get_composer_state", { taskId });
}

export function setComposerState(state: ChatComposerState): Promise<void> {
  return invoke<void>("chat_set_composer_state", { state });
}

/** 让下一次发送从全新 SDK session 开始（同时清掉前端可见的消息历史）。 */
export function resetSession(taskId: string): Promise<void> {
  return invoke<void>("chat_reset_session", { taskId });
}

/** 健康检查：API key 是否在环境变量里、node 是否能跑。 */
export function checkEnv(): Promise<EnvStatus> {
  return invoke<EnvStatus>("chat_check_env");
}

// ---- 事件订阅 ----
//
// 注意：每个 listen 各自返回 unlisten；调用方在 onUnmounted 里需要全部 await。

export function onChunk(handler: (e: ChunkEvent) => void): Promise<UnlistenFn> {
  return listen<ChunkEvent>("chat:chunk", (event) => handler(event.payload));
}

export function onTool(handler: (e: ToolEvent) => void): Promise<UnlistenFn> {
  return listen<ToolEvent>("chat:tool", (event) => handler(event.payload));
}

export function onDone(handler: (e: DoneEvent) => void): Promise<UnlistenFn> {
  return listen<DoneEvent>("chat:done", (event) => handler(event.payload));
}

export function onError(handler: (e: ErrorEvent) => void): Promise<UnlistenFn> {
  return listen<ErrorEvent>("chat:error", (event) => handler(event.payload));
}
