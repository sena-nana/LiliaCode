import type { ChatBackendKind } from "./chat";

export type RouterMode = "cc-switch" | "direct";

export type CodexReasoningEffort = "low" | "medium" | "high" | "xhigh";

export type CodexSettingsProfile = "default" | "fast" | "balanced" | "deep";

export type CodexPermissionProfile =
  | "default"
  | "readOnly"
  | "workspaceWrite"
  | "dangerFullAccess";

export interface CodexControlledPermissions {
  profile: CodexPermissionProfile;
}

export interface CodexProfileSettings {
  profile: CodexSettingsProfile;
  model: string | null;
  reasoningEffort: CodexReasoningEffort | null;
  runtimeWorkspaceRoots: string[];
  permissions: CodexControlledPermissions;
}

export interface CodexComposerSettings {
  profile?: CodexSettingsProfile | null;
  model?: string | null;
  reasoningEffort?: CodexReasoningEffort | null;
  runtimeWorkspaceRoots?: string[] | null;
  permissions?: CodexControlledPermissions | null;
}

export interface ProviderConfig {
  backend: ChatBackendKind;
  baseUrl: string | null;
  apiKey: string | null;
}

export interface CCSwitchConfig {
  baseUrl: string | null;
}

export interface AssistantAIConfig {
  baseUrl: string | null;
  apiKey: string | null;
  model: string | null;
}

export interface AssistantAITestResult {
  ok: boolean;
  error: string | null;
  models: string[] | null;
  modelMatched: boolean | null;
}

export type ConnectionMode = "cc-switch" | "custom" | "direct" | "unconfigured";

export interface BackendEnvStatus {
  backend: ChatBackendKind;
  hasApiKey: boolean;
  connectionMode: ConnectionMode;
  effectiveUrl: string | null;
}

export interface CCSwitchStatus {
  reachable: boolean;
  baseUrl: string | null;
}

export interface CodexAppServerStatus {
  version: string | null;
  available: boolean;
  supportsRequiredProtocol: boolean;
  failureKind:
    | "missingCli"
    | "appServerUnavailable"
    | "experimentalApiUnsupported"
    | "providerIncompatible"
    | null;
  issues: string[];
}

export interface EnvStatusReport {
  nodeAvailable: boolean;
  codexCliAvailable: boolean;
  codexAppServer: CodexAppServerStatus;
  ccSwitch: CCSwitchStatus;
  routerModes: Record<ChatBackendKind, RouterMode>;
  backends: Record<ChatBackendKind, BackendEnvStatus>;
}
