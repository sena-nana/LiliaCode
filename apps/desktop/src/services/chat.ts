/**
 * Chat 服务层：把 Tauri command/event 包成 typed 函数。
 * 输入/输出形状走 @lilia/contracts，跨端共享；Rust 侧 `#[serde(rename_all = "camelCase")]`
 * 已对齐字段名，前端不需再做 key 映射。
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AssistantAIConfig,
  AssistantAITestResult,
  BackendEnvStatus,
  CCSwitchConfig,
  CCSwitchStatus,
  ChatBackendKind,
  ChatAttachment,
  ChatComposerState,
  AgentTimelineEvent,
  AgentAskUserRequestEvent,
  AskUserResult,
  ChatModelOption,
  ChatSendResult,
  ConnectionMode,
  EnvStatusReport,
  ProviderConfig,
  RouterMode,
} from "@lilia/contracts";

export type {
  AssistantAIConfig,
  AssistantAITestResult,
  ChatAttachment,
  ConnectionMode,
  BackendEnvStatus,
  CCSwitchConfig,
  CCSwitchStatus,
  EnvStatusReport,
  ProviderConfig,
  RouterMode,
};

export interface TurnStartedEvent { taskId: string; queuedCount: number; }
export interface DoneEvent { taskId: string; sessionId: string | null; subtype: string | null; }

/**
 * runner 通过 canUseTool 把工具调用授权请求转过来，等用户决策。
 * 字段对齐 Claude SDK 的 CanUseTool 入参：title / description / displayName
 * 由 SDK bridge 在能拿到时填好；input 是原始工具入参 JSON。
 */
export interface ToolConsentRequest {
  taskId: string;
  turnId: string;
  backend: ChatBackendKind;
  requestId: string;
  toolName: string;
  input: Record<string, unknown>;
  title: string | null;
  displayName: string | null;
  description: string | null;
  blockedPath: string | null;
  decisionReason: string | null;
  toolUseId: string | null;
}

export type ToolConsentDecision = "allow" | "deny";

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
): Promise<ChatSendResult> {
  return invoke<ChatSendResult>("chat_send_message", {
    taskId,
    content,
    composer,
    projectCwd,
    attachments,
  });
}

export function interruptTurn(taskId: string): Promise<void> {
  return invoke<void>("chat_interrupt_turn", { taskId });
}

export function describeAttachments(paths: string[]): Promise<ChatAttachment[]> {
  return invoke<ChatAttachment[]>("chat_describe_attachments", { paths });
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

export function listModels(backend: ChatBackendKind): Promise<ChatModelOption[]> {
  return invoke<ChatModelOption[]>("chat_list_models", { backend });
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

export function onAskUserRequest(
  handler: (e: AgentAskUserRequest) => void,
): Promise<UnlistenFn> {
  return listen<AgentAskUserRequest>("chat:ask-user-request", (event) =>
    handler(event.payload),
  );
}

/** 把用户对一次 canUseTool 的决策写回 runner，让被卡住的工具继续 / 终止。 */
export function respondToolConsent(
  taskId: string,
  requestId: string,
  decision: ToolConsentDecision,
  message?: string,
): Promise<void> {
  return invoke<void>("chat_respond_tool_consent", {
    taskId,
    requestId,
    decision,
    message: message ?? null,
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
