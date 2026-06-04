import type { ChatBackendKind } from "./chat";

export type RouterMode = "cc-switch" | "direct";

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
