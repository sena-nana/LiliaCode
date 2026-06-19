import type { ChatBackendKind } from "./chat";

export type ProviderConnectionMode = "api" | "codex-account";
export type RouterMode = ProviderConnectionMode;

export type CodexReasoningEffort = "low" | "medium" | "high" | "xhigh";

export type CodexSettingsProfile = "default" | "fast" | "balanced" | "deep";

export type CodexJsonObject = Record<string, unknown>;

export interface CodexProfileSettings {
  profile: CodexSettingsProfile;
  model: string | null;
  reasoningEffort: CodexReasoningEffort | null;
  runtimeWorkspaceRoots: string[];
  responsesApiClientMetadata: CodexJsonObject | null;
  additionalContext: string | null;
  persistExtendedHistory: boolean | null;
  initialTurnsPage: CodexJsonObject | null;
  excludeTurns: string[];
}

export interface CodexComposerSettings {
  profile?: CodexSettingsProfile | null;
  model?: string | null;
  reasoningEffort?: CodexReasoningEffort | null;
  runtimeWorkspaceRoots?: string[] | null;
  responsesApiClientMetadata?: CodexJsonObject | null;
  additionalContext?: string | null;
  persistExtendedHistory?: boolean | null;
  initialTurnsPage?: CodexJsonObject | null;
  excludeTurns?: string[] | null;
}

export interface ProviderConfig {
  backend: ChatBackendKind;
  baseUrl: string | null;
  apiKey: string | null;
  hasApiKey: boolean;
  clearApiKey?: boolean;
}

export interface AssistantAIConfig {
  baseUrl: string | null;
  apiKey: string | null;
  model: string | null;
  hasApiKey: boolean;
  clearApiKey?: boolean;
}

export interface AssistantAITestResult {
  ok: boolean;
  error: string | null;
  models: string[] | null;
  modelMatched: boolean | null;
}

export type ConnectionMode = "api" | "custom" | "codex-account" | "unconfigured";

export interface BackendEnvStatus {
  backend: ChatBackendKind;
  hasApiKey: boolean;
  connectionMode: ConnectionMode;
  effectiveUrl: string | null;
}

export interface CodexAppServerStatus {
  version: string | null;
  installPath: string | null;
  managed: boolean;
  available: boolean;
  supportsRequiredProtocol: boolean;
  failureKind:
    | "missingCli"
    | "appServerUnavailable"
    | "experimentalApiUnsupported"
    | "providerIncompatible"
    | null;
  issues: string[];
  latestVersion: string | null;
  updateAvailable: boolean;
  releaseNotes: string[];
  updateError: string | null;
}

export interface EnvStatusReport {
  nodeAvailable: boolean;
  codexCliAvailable: boolean;
  codexAppServer: CodexAppServerStatus;
  routerModes: Record<ChatBackendKind, RouterMode>;
  backends: Record<ChatBackendKind, BackendEnvStatus>;
}
