export type RuntimeSettingsAction = "diagnose" | "update";
export type RemoteEnvironmentAction = "diagnose" | "add" | "select";

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
}

export interface RemoteEnvironmentRuntimeCommand {
  type: "remote_environment";
  action: RemoteEnvironmentAction;
  environmentId?: string;
  environment?: Record<string, unknown>;
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
  };
};

export const RUNTIME_SETTINGS_COMMAND_TYPE: "runtime_settings";
export const RUNTIME_SETTINGS_ACTIONS: readonly RuntimeSettingsAction[];
export const REMOTE_ENVIRONMENT_COMMAND_TYPE: "remote_environment";
export const REMOTE_ENVIRONMENT_ACTIONS: readonly RemoteEnvironmentAction[];
export const SANDBOX_DIAGNOSTICS_COMMAND_TYPE: "sandbox_diagnostics";
export const SESSION_FORK_COMMAND_TYPE: "session_fork";
export const DEFAULT_SESSION_FORK_EXCLUDE_TURNS: boolean;

export function isRuntimeSettingsAction(value: unknown): value is RuntimeSettingsAction;

export function isRemoteEnvironmentAction(value: unknown): value is RemoteEnvironmentAction;

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
};
