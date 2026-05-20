/**
 * Chat 服务层：把 Tauri command/event 包成 typed 函数，Vue 组件不直接碰
 * @tauri-apps/api。
 *
 * - 输入/输出形状全部走 @lilia/contracts，跨端共享。
 * - Rust 那侧用 `#[serde(rename_all = "camelCase")]`，所以这里不需要再做 key 映射。
 * - 订阅事件 `chat:message`：assistant 的异步回复会通过它推回，组件只需把回调
 *   挂上去，离开页面时记得调 unlisten。
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ChatBranchOption,
  ChatComposerState,
  ChatMessage,
  ChatModelOption,
} from "@lilia/contracts";

export function listMessages(taskId: string): Promise<ChatMessage[]> {
  return invoke<ChatMessage[]>("chat_list_messages", { taskId });
}

export function sendMessage(
  taskId: string,
  content: string,
  composer: ChatComposerState,
): Promise<ChatMessage> {
  return invoke<ChatMessage>("chat_send_message", { taskId, content, composer });
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

/**
 * 订阅 assistant 回复事件。多个组件可以同时订阅，每次返回一个 unlisten 函数；
 * 调用方负责在 onUnmounted 里 await unlisten()，避免回调泄漏到下一个会话。
 */
export function onAssistantMessage(
  handler: (msg: ChatMessage) => void,
): Promise<UnlistenFn> {
  return listen<ChatMessage>("chat:message", (event) => handler(event.payload));
}
