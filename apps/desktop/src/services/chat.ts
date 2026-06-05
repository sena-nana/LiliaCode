/**
 * Chat 服务层：把 Tauri command/event 包成 typed 函数。
 * 输入/输出形状走 @lilia/contracts，跨端共享；Rust 侧 `#[serde(rename_all = "camelCase")]`
 * 已对齐字段名，前端不需再做 key 映射。
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AgentInteractionSettings,
  AgentInteractionKind,
  AgentInteractionRequest,
  AgentInteractionResponse,
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
  ChatSendResult,
  ConnectionMode,
  EnvStatusReport,
  ProviderConfig,
  RouterMode,
  SuggestionItem,
  SuggestionSettings,
  ToolConsentDecision,
  ToolConsentRequest,
  ToolConsentUpdatedInput,
} from "@lilia/contracts";

export type {
  AgentInteractionSettings,
  AgentInteractionKind,
  AgentInteractionRequest,
  AgentInteractionResponse,
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
  SuggestionItem,
  SuggestionSettings,
  ToolConsentDecision,
  ToolConsentRequest,
  ToolConsentUpdatedInput,
};

export interface TurnStartedEvent { taskId: string; queuedCount: number; }
export interface DoneEvent { taskId: string; sessionId: string | null; subtype: string | null; }

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
export function checkEnv(options: { forceRefresh?: boolean } = {}): Promise<EnvStatusReport> {
  return invoke<EnvStatusReport>("chat_check_env", {
    forceRefresh: options.forceRefresh ?? false,
  });
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

export function getConversationSuggestions(
  projectId?: string | null,
  forceRefresh = false,
): Promise<SuggestionItem[]> {
  return invoke<SuggestionItem[]>("conversation_suggestions_get", {
    projectId: projectId ?? null,
    forceRefresh,
  });
}

export function getConversationSuggestionSettings(): Promise<SuggestionSettings> {
  return invoke<SuggestionSettings>("conversation_suggestions_get_settings");
}

export function setConversationSuggestionSettings(
  settings: SuggestionSettings,
): Promise<void> {
  return invoke<void>("conversation_suggestions_set_settings", { settings });
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

function normalizeAgentInteractionRequest(value: AgentInteractionRequest): AgentInteractionRequest | null {
  const row = value as unknown as Record<string, unknown>;
  const payload = row.payload;
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const taskId = stringField(row, "taskId", "task_id");
  const turnId = stringField(row, "turnId", "turn_id");
  const requestId = stringField(row, "requestId", "request_id");
  const backend = stringField(row, "backend", "backend");
  const kind = stringField(row, "kind", "kind");
  if (!taskId || !requestId) return null;
  if (kind !== "plan_approval" && kind !== "tool_consent" && kind !== "ask_user") return null;
  return {
    taskId,
    turnId,
    backend: backend === "codex" ? "codex" : "claude",
    requestId,
    kind,
    payload,
  } as AgentInteractionRequest;
}

export function onAgentInteractionRequest(
  handler: (e: AgentInteractionRequest) => void,
): Promise<UnlistenFn> {
  return listen<AgentInteractionRequest>("chat:agent-interaction-request", (event) => {
    const req = normalizeAgentInteractionRequest(event.payload);
    if (req) handler(req);
  });
}

function stringField(row: Record<string, unknown>, camel: string, snake: string): string {
  const value = row[camel] ?? row[snake];
  return typeof value === "string" ? value : "";
}

export function respondAgentInteraction(response: AgentInteractionResponse): Promise<void> {
  return invoke<void>("chat_respond_agent_interaction", {
    taskId: response.taskId,
    requestId: response.requestId,
    kind: response.kind,
    result: response.result,
  });
}
