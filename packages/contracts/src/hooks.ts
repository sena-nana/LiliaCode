export type HookBackendKind = "claude" | "codex";

export type HookScope =
  | "managed"
  | "user"
  | "project"
  | "local"
  | "plugin"
  | "system";

export type HookSourceFormat =
  | "claude_settings_json"
  | "codex_hooks_json"
  | "codex_config_toml"
  | "managed_settings"
  | "requirements_toml"
  | "plugin_manifest";

export type HookTrustState = "unknown" | "required" | "managed" | "n_a";

export interface HookSourceSummary {
  id: string;
  backend: HookBackendKind;
  scope: HookScope;
  format: HookSourceFormat;
  name: string;
  path: string;
  exists: boolean;
  editable: boolean;
  managed: boolean;
  enabled: boolean;
  handlerCount: number;
  warnings: string[];
  limitations: string[];
  trustState: HookTrustState;
  description?: string | null;
}

export interface HookHandlerView {
  id: string;
  event: string;
  matcher: string | null;
  type: string;
  command: string | null;
  commandWindows: string | null;
  timeoutSeconds: number | null;
  statusMessage: string | null;
  supported: boolean;
  executable: boolean;
  groupAdvancedJson: string | null;
  advancedJson: string | null;
  warnings: string[];
}

export interface HookDocumentView {
  source: HookSourceSummary;
  handlers: HookHandlerView[];
  rawDocument: string | null;
  rawFormat: "json" | "toml" | "text";
  warnings: string[];
  limitations: string[];
}

export interface HooksOverview {
  sources: HookSourceSummary[];
  warnings: string[];
}

export interface HookHandlerUpdateInput {
  id?: string;
  event: string;
  matcher?: string | null;
  type: string;
  command?: string | null;
  commandWindows?: string | null;
  timeoutSeconds?: number | null;
  statusMessage?: string | null;
  groupAdvancedJson?: string | null;
  advancedJson?: string | null;
}

export interface HookDocumentUpdateInput {
  handlers: HookHandlerUpdateInput[];
}
