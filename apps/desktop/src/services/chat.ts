/**
 * Chat 服务层：把 Tauri command/event 包成 typed 函数。
 * 输入/输出形状走 @lilia/contracts，跨端共享；Rust 侧 `#[serde(rename_all = "camelCase")]`
 * 已对齐字段名，前端不需再做 key 映射。
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AgentInteractionSettings,
  AssistantAIConfig,
  AssistantAITestResult,
  BackendEnvStatus,
  CCSwitchConfig,
  CCSwitchStatus,
  ChatBackendKind,
  ChatAttachment,
  ChatContextSearchResult,
  ChatComposerState,
  AgentTimelineEvent,
  AgentAskUserRequestEvent,
  AskUserResult,
  ChatSendResult,
  ConnectionMode,
  EnvStatusReport,
  ProviderConfig,
  RouterMode,
  ToolConsentDecision,
  ToolConsentRequest,
  ToolConsentUpdatedInput,
} from "@lilia/contracts";

export type {
  AgentInteractionSettings,
  AssistantAIConfig,
  AssistantAITestResult,
  ChatAttachment,
  ChatContextSearchResult,
  ConnectionMode,
  BackendEnvStatus,
  CCSwitchConfig,
  CCSwitchStatus,
  EnvStatusReport,
  ProviderConfig,
  RouterMode,
  ToolConsentDecision,
  ToolConsentRequest,
  ToolConsentUpdatedInput,
};

export interface TurnStartedEvent { taskId: string; queuedCount: number; }
export interface DoneEvent { taskId: string; sessionId: string | null; subtype: string | null; }

export type AgentAskUserRequest = AgentAskUserRequestEvent;

export function listAgentTimeline(taskId: string): Promise<AgentTimelineEvent[]> {
  return invoke<AgentTimelineEvent[]>("agent_timeline_list", { taskId });
}

/**
 * 发起一轮对话。返回值是 user 那条消息本身（用于乐观渲染）；
 * Agent 的过程与最终回复通过 agent timeline 异步推回。
 * projectCwd 决定 agent 能看到的文件树。
 */
export function sendMessage(
  taskId: string,
  content: string,
  composer: ChatComposerState,
  projectCwd: string,
  attachments: ChatAttachment[] = [],
  guideId?: string,
): Promise<ChatSendResult> {
  return invoke<ChatSendResult>("chat_send_message", {
    taskId,
    content,
    composer,
    projectCwd,
    attachments,
    guideId: guideId ?? null,
  });
}

export function interruptTurn(taskId: string): Promise<void> {
  return invoke<void>("chat_interrupt_turn", { taskId });
}

export function describeAttachments(paths: string[]): Promise<ChatAttachment[]> {
  return invoke<ChatAttachment[]>("chat_describe_attachments", { paths });
}

export function searchContextAttachments(
  projectCwd: string,
  query: string,
  limit = 12,
): Promise<ChatContextSearchResult[]> {
  return invoke<ChatContextSearchResult[]>("chat_search_context_attachments", {
    projectCwd,
    query,
    limit,
  });
}

export function readClipboardFilePaths(): Promise<string[]> {
  return invoke<string[]>("chat_read_clipboard_file_paths");
}

export function saveClipboardImage(input: {
  mime: string | null;
  bytesBase64: string;
  name?: string | null;
}): Promise<ChatAttachment> {
  return invoke<ChatAttachment>("chat_save_clipboard_image", { input });
}

export function saveClipboardText(input: { text: string }): Promise<ChatAttachment> {
  return invoke<ChatAttachment>("chat_save_clipboard_text", { input });
}

export async function pickAttachmentFiles(): Promise<string[]> {
  const picked = await invoke<string | string[] | null>("plugin:dialog|open", {
    options: {
      directory: false,
      multiple: true,
      title: "选择附件",
    },
  });
  if (!picked) return [];
  return Array.isArray(picked) ? picked : [picked];
}

export function getComposerState(taskId: string): Promise<ChatComposerState> {
  return invoke<ChatComposerState>("chat_get_composer_state", { taskId });
}

export function setComposerState(state: ChatComposerState): Promise<void> {
  return invoke<void>("chat_set_composer_state", { state });
}

export function getAgentInteractionSettings(): Promise<AgentInteractionSettings> {
  return invoke<AgentInteractionSettings>("agent_interaction_get_settings");
}

export function setAgentInteractionSettings(
  settings: Partial<AgentInteractionSettings>,
): Promise<void> {
  return invoke<void>("agent_interaction_set_settings", { settings });
}

/** 让下一次发送从全新 SDK session 开始（同时清掉前端可见的消息历史）。 */
export function resetSession(taskId: string): Promise<void> {
  return invoke<void>("chat_reset_session", { taskId });
}

/** 健康检查：node / codex CLI 是否在 PATH，两个 backend 当前的连接模式。 */
export function checkEnv(): Promise<EnvStatusReport> {
  return invoke<EnvStatusReport>("chat_check_env");
}

export function getProviderConfig(backend: ChatBackendKind): Promise<ProviderConfig> {
  return invoke<ProviderConfig>("provider_get_config", { backend });
}

export function setProviderConfig(config: ProviderConfig): Promise<void> {
  return invoke<void>("provider_set_config", { config });
}

export function getActiveBackend(): Promise<ChatBackendKind> {
  return invoke<ChatBackendKind>("provider_get_active_backend");
}

export function setActiveBackend(backend: ChatBackendKind): Promise<void> {
  return invoke<void>("provider_set_active_backend", { backend });
}

export function getCCSwitchConfig(): Promise<CCSwitchConfig> {
  return invoke<CCSwitchConfig>("cc_switch_get_config");
}

export function setCCSwitchConfig(config: CCSwitchConfig): Promise<void> {
  return invoke<void>("cc_switch_set_config", { config });
}

export function getRouterMode(backend: ChatBackendKind): Promise<RouterMode> {
  return invoke<RouterMode>("router_get_mode", { backend });
}

export function setRouterMode(backend: ChatBackendKind, mode: RouterMode): Promise<void> {
  return invoke<void>("router_set_mode", { backend, mode });
}

// ---- 辅助模型（Assistant AI） ----
// 与 Provider 平级、独立配置，不参与 Agent 主循环；供 Memory 助手等周边模块消费。

export function getAssistantAIConfig(): Promise<AssistantAIConfig> {
  return invoke<AssistantAIConfig>("assistant_ai_get_config");
}

export function setAssistantAIConfig(config: AssistantAIConfig): Promise<void> {
  return invoke<void>("assistant_ai_set_config", { config });
}

export function testAssistantAIConnection(
  config: AssistantAIConfig,
): Promise<AssistantAITestResult> {
  return invoke<AssistantAITestResult>("assistant_ai_test_connection", { config });
}

// ---- 事件订阅 ----
// UI 只订阅 turn 状态和 agent timeline；文本、工具、错误都归入 timeline 事件。

export function onTurnStarted(handler: (e: TurnStartedEvent) => void): Promise<UnlistenFn> {
  return listen<TurnStartedEvent>("chat:turn-started", (event) => handler(event.payload));
}

export function onDone(handler: (e: DoneEvent) => void): Promise<UnlistenFn> {
  return listen<DoneEvent>("chat:done", (event) => handler(event.payload));
}

export function onAgentTimeline(
  handler: (e: AgentTimelineEvent) => void,
): Promise<UnlistenFn> {
  return listen<AgentTimelineEvent>("agent:timeline", (event) => handler(event.payload));
}

export function onToolConsentRequest(
  handler: (e: ToolConsentRequest) => void,
): Promise<UnlistenFn> {
  return listen<ToolConsentRequest>("chat:tool-consent-request", (event) =>
    handler(event.payload),
  );
}

function stringField(row: Record<string, unknown>, camel: string, snake: string): string {
  const value = row[camel] ?? row[snake];
  return typeof value === "string" ? value : "";
}

function normalizeAskUserRequest(value: AgentAskUserRequest): AgentAskUserRequest | null {
  const row = value as unknown as Record<string, unknown>;
  const spec = row.spec;
  if (!spec || typeof spec !== "object" || Array.isArray(spec)) return null;
  const taskId = stringField(row, "taskId", "task_id");
  const requestId = stringField(row, "requestId", "request_id");
  if (!taskId || !requestId) return null;
  return {
    taskId,
    turnId: stringField(row, "turnId", "turn_id"),
    backend: stringField(row, "backend", "backend") as AgentAskUserRequest["backend"],
    requestId,
    spec: spec as AgentAskUserRequest["spec"],
  };
}

export function onAskUserRequest(
  handler: (e: AgentAskUserRequest) => void,
): Promise<UnlistenFn> {
  return listen<AgentAskUserRequest>("chat:ask-user-request", (event) => {
    const req = normalizeAskUserRequest(event.payload);
    if (req) handler(req);
  });
}

/** 把用户对一次 canUseTool 的决策写回 runner，让被卡住的工具继续 / 终止。 */
export function respondToolConsent(
  taskId: string,
  requestId: string,
  decision: ToolConsentDecision,
  message?: string,
  updatedInput?: ToolConsentUpdatedInput,
): Promise<void> {
  return invoke<void>("chat_respond_tool_consent", {
    taskId,
    requestId,
    decision,
    message: message ?? null,
    updatedInput: updatedInput ?? null,
  });
}

/** 把 AskUser 浮层收集到的结果写回 runner，让 Claude AskUserQuestion 继续。 */
export function respondAskUser(
  taskId: string,
  requestId: string,
  result: AskUserResult,
): Promise<void> {
  return invoke<void>("chat_respond_ask_user", {
    taskId,
    requestId,
    result,
  });
}
