export type RuntimePermissionMode = "full" | "ask" | "readonly" | "free";
export type RuntimePermissionBackend = "claude" | "codex";

export interface PermissionModeDisplay {
  label: string;
  description: string;
}

export interface ClaudePermissionRuntimeMapping {
  permissionMode: "default" | "bypassPermissions";
  allowDangerouslySkipPermissions: boolean;
}

export interface CodexPermissionRuntimeMapping {
  sandboxMode: "danger-full-access" | "workspace-write" | "read-only";
  sandboxPolicy: "dangerFullAccess" | "workspaceWrite" | "readOnly";
  approvalPolicy: "never" | "on-request";
  permissionProfile: "dangerFullAccess" | "workspaceWrite" | "readOnly";
  permissionProfileId: ":danger-no-sandbox" | ":workspace" | ":read-only";
}

export const PERMISSION_MODES_MANIFEST: {
  permissionModes: readonly RuntimePermissionMode[];
  defaultPermissionMode: RuntimePermissionMode;
  display: Readonly<Record<RuntimePermissionMode, PermissionModeDisplay>>;
  displayOrder: readonly RuntimePermissionMode[];
  runtimeMappings: {
    claude: Record<RuntimePermissionMode, ClaudePermissionRuntimeMapping>;
    codex: Record<RuntimePermissionMode, CodexPermissionRuntimeMapping>;
  };
};

export const PERMISSION_MODES: readonly RuntimePermissionMode[];
export const DEFAULT_PERMISSION_MODE: RuntimePermissionMode;
export const PERMISSION_MODE_DISPLAY: Readonly<Record<RuntimePermissionMode, PermissionModeDisplay>>;
export const PERMISSION_MODE_DISPLAY_ORDER: readonly RuntimePermissionMode[];

export function isRuntimePermissionMode(value: unknown): value is RuntimePermissionMode;

export function normalizeRuntimePermissionMode(
  value: unknown,
  fallback?: RuntimePermissionMode,
): RuntimePermissionMode;

export function runtimePermissionMapping(
  backend: "claude",
  permission: unknown,
): ClaudePermissionRuntimeMapping | null;

export function runtimePermissionMapping(
  backend: "codex",
  permission: unknown,
): CodexPermissionRuntimeMapping | null;

export function runtimePermissionMapping(
  backend: RuntimePermissionBackend,
  permission: unknown,
): ClaudePermissionRuntimeMapping | CodexPermissionRuntimeMapping | null;

export function claudePermissionRuntime(permission: unknown): ClaudePermissionRuntimeMapping | null;

export function codexPermissionRuntime(permission: unknown): CodexPermissionRuntimeMapping | null;
