export type RuntimeSettingsAction = "diagnose" | "update";
export type RemoteEnvironmentAction = "diagnose" | "add" | "select";
export type SessionForkMode = "continue" | "fork";
export type ProcessSessionAction = "spawn" | "write_stdin" | "kill" | "resize_pty";

export interface NormalizedRuntimeSettingsCommand {
  action: RuntimeSettingsAction;
}

export interface NormalizedRemoteEnvironmentCommand {
  action: RemoteEnvironmentAction;
  environmentId: string;
  environment: Record<string, unknown> | null;
}

export interface NormalizedSandboxDiagnosticsCommand {
  includeDetails: boolean;
}

export interface NormalizedSessionForkCommand {
  excludeTurns: boolean;
  sourceTurnId: string;
  mode: SessionForkMode;
}

export interface NormalizedProcessSessionCommand {
  action: ProcessSessionAction;
  processId: string;
  command: string;
  cwd: string;
  stdin: string;
  rows: number | null;
  cols: number | null;
  env: Record<string, string> | null;
  tty: boolean;
  permissionProfile: string;
}

export interface RemoteEnvironmentRuntimeCommand {
  type: "remote_environment";
  action: RemoteEnvironmentAction;
  environmentId?: string;
  environment?: Record<string, unknown>;
}

export interface ProcessSessionRuntimeCommand {
  type: "process_session";
  action: ProcessSessionAction;
  processId?: string;
  command?: string;
  cwd?: string;
  stdin?: string;
  rows?: number;
  cols?: number;
  env?: Record<string, string>;
  tty?: boolean;
  permissionProfile?: string;
}

export const RUNTIME_COMMAND_CONTRACT: {
  runtimeSettings: {
    type: "runtime_settings";
    actions: readonly RuntimeSettingsAction[];
  };
  remoteEnvironment: {
    type: "remote_environment";
    actions: readonly RemoteEnvironmentAction[];
    requiresEnvironment: readonly RemoteEnvironmentAction[];
    requiresEnvironmentId: readonly RemoteEnvironmentAction[];
  };
  sandboxDiagnostics: {
    type: "sandbox_diagnostics";
  };
  sessionFork: {
    type: "session_fork";
    defaultExcludeTurns: boolean;
    modes: readonly SessionForkMode[];
    defaultMode: SessionForkMode;
  };
  processSession: {
    type: "process_session";
    actions: readonly ProcessSessionAction[];
    requiresCommand: readonly ProcessSessionAction[];
  };
};

export const RUNTIME_SETTINGS_COMMAND_TYPE: "runtime_settings";
export const RUNTIME_SETTINGS_ACTIONS: readonly RuntimeSettingsAction[];
export const REMOTE_ENVIRONMENT_COMMAND_TYPE: "remote_environment";
export const REMOTE_ENVIRONMENT_ACTIONS: readonly RemoteEnvironmentAction[];
export const SANDBOX_DIAGNOSTICS_COMMAND_TYPE: "sandbox_diagnostics";
export const SESSION_FORK_COMMAND_TYPE: "session_fork";
export const DEFAULT_SESSION_FORK_EXCLUDE_TURNS: boolean;
export const SESSION_FORK_MODES: readonly SessionForkMode[];
export const DEFAULT_SESSION_FORK_MODE: SessionForkMode;
export const PROCESS_SESSION_COMMAND_TYPE: "process_session";
export const PROCESS_SESSION_ACTIONS: readonly ProcessSessionAction[];

export function isRuntimeSettingsAction(value: unknown): value is RuntimeSettingsAction;

export function isRemoteEnvironmentAction(value: unknown): value is RemoteEnvironmentAction;

export function isSessionForkMode(value: unknown): value is SessionForkMode;

export function isProcessSessionAction(value: unknown): value is ProcessSessionAction;

export function normalizeRuntimeSettingsCommand(
  value: unknown,
): NormalizedRuntimeSettingsCommand | null;

export function createRuntimeSettingsCommand(action: RuntimeSettingsAction): {
  type: "runtime_settings";
  action: RuntimeSettingsAction;
};

export function normalizeRemoteEnvironmentCommand(
  value: unknown,
): NormalizedRemoteEnvironmentCommand | null;

export function createRemoteEnvironmentCommand(
  action: RemoteEnvironmentAction,
  options?: Partial<Omit<RemoteEnvironmentRuntimeCommand, "type" | "action">>,
): RemoteEnvironmentRuntimeCommand;

export function normalizeSandboxDiagnosticsCommand(
  value: unknown,
): NormalizedSandboxDiagnosticsCommand | null;

export function createSandboxDiagnosticsCommand(
  options?: Partial<NormalizedSandboxDiagnosticsCommand>,
): {
  type: "sandbox_diagnostics";
  includeDetails: boolean;
};

export function normalizeSessionForkCommand(
  value: unknown,
): NormalizedSessionForkCommand | null;

export function createSessionForkCommand(
  options?: Partial<NormalizedSessionForkCommand>,
): {
  type: "session_fork";
  excludeTurns: boolean;
  sourceTurnId?: string;
  mode: SessionForkMode;
};

export function normalizeProcessSessionCommand(
  value: unknown,
): NormalizedProcessSessionCommand | null;

export function createProcessSessionCommand(
  action: ProcessSessionAction,
  options?: Partial<Omit<ProcessSessionRuntimeCommand, "type" | "action">>,
): ProcessSessionRuntimeCommand;
