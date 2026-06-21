export type ChatBackendKind = "claude" | "codex";
export type ReasoningEffort = "low" | "medium" | "high" | "xhigh" | "max";
export type CodexReasoningEffort = "low" | "medium" | "high" | "xhigh";
export type RouterMode = "api" | "codex-account";
export type ConnectionMode = "api" | "custom" | "codex-account" | "unconfigured";

export interface ChatModelOptionContract {
  id: string;
  label: string;
  backend: ChatBackendKind;
}

export const CHAT_BACKENDS_CONTRACT: Record<string, unknown>;
export const CHAT_BACKENDS: readonly ["claude", "codex"];
export const DEFAULT_CHAT_BACKEND: ChatBackendKind;
export const CHAT_BACKEND_LABELS: Readonly<Record<ChatBackendKind, string>>;
export const DEFAULT_MODEL_BY_BACKEND: Readonly<Record<ChatBackendKind, string>>;
export const MODEL_OPTIONS_BY_BACKEND: Readonly<
  Record<ChatBackendKind, readonly ChatModelOptionContract[]>
>;
export const ALLOWED_MODEL_PREFIXES_BY_BACKEND: Readonly<
  Record<ChatBackendKind, readonly string[]>
>;
export const REASONING_EFFORTS: readonly [
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
];
export const BACKEND_REASONING_EFFORTS: Readonly<
  Record<ChatBackendKind, readonly ReasoningEffort[]>
>;
export const CODEX_REASONING_EFFORTS: readonly [
  "low",
  "medium",
  "high",
  "xhigh",
];
export const DIRECT_DEFAULT_URLS: Readonly<Record<ChatBackendKind, string>>;
export const API_KEY_ENV_BY_BACKEND: Readonly<Record<ChatBackendKind, string>>;
export const PROVIDER_STORE_KEY_BY_BACKEND: Readonly<
  Record<ChatBackendKind, string>
>;
export const ROUTER_STORE_KEY_BY_BACKEND: Readonly<
  Record<ChatBackendKind, string>
>;
export const ROUTER_MODES_BY_BACKEND: Readonly<
  Record<ChatBackendKind, readonly RouterMode[]>
>;
export const DEFAULT_ROUTER_MODE_BY_BACKEND: Readonly<
  Record<ChatBackendKind, RouterMode>
>;
export const ROUTER_MODE_LABELS: Readonly<Record<RouterMode, string>>;
export const ROUTER_MODES_USING_API_CONFIG: readonly RouterMode[];
export const ROUTER_MODES_USING_CODEX_ACCOUNT: readonly RouterMode[];
export const API_DESCRIPTION_BY_BACKEND: Readonly<Record<ChatBackendKind, string>>;
export const CONNECTION_MODES: readonly [
  "api",
  "custom",
  "codex-account",
  "unconfigured",
];
export const CONNECTION_MODES_USING_API_KEY: readonly ConnectionMode[];
export const CONNECTION_MODES_USING_DEFAULT_API: readonly ConnectionMode[];
export const CONNECTION_MODES_USING_CUSTOM_URL: readonly ConnectionMode[];
export const CONNECTION_MODES_USING_CODEX_ACCOUNT: readonly ConnectionMode[];
export const UNCONFIGURED_CONNECTION_MODES: readonly ConnectionMode[];
