import runtimeCommandContract from "./runtime-command-contract.json" with { type: "json" };

const manifest = deepFreeze(runtimeCommandContract);

export const RUNTIME_COMMAND_CONTRACT = manifest;
export const RUNTIME_SETTINGS_COMMAND_TYPE = manifest.runtimeSettings.type;
export const RUNTIME_SETTINGS_ACTIONS = manifest.runtimeSettings.actions;
export const REMOTE_ENVIRONMENT_COMMAND_TYPE = manifest.remoteEnvironment.type;
export const REMOTE_ENVIRONMENT_ACTIONS = manifest.remoteEnvironment.actions;
export const SANDBOX_DIAGNOSTICS_COMMAND_TYPE = manifest.sandboxDiagnostics.type;
export const SESSION_FORK_COMMAND_TYPE = manifest.sessionFork.type;
export const DEFAULT_SESSION_FORK_EXCLUDE_TURNS = manifest.sessionFork.defaultExcludeTurns;
export const SESSION_FORK_MODES = manifest.sessionFork.modes;
export const DEFAULT_SESSION_FORK_MODE = manifest.sessionFork.defaultMode;
export const PROCESS_SESSION_COMMAND_TYPE = manifest.processSession.type;
export const PROCESS_SESSION_ACTIONS = manifest.processSession.actions;

const runtimeSettingsActionSet = new Set(RUNTIME_SETTINGS_ACTIONS);
const remoteEnvironmentActionSet = new Set(REMOTE_ENVIRONMENT_ACTIONS);
const sessionForkModeSet = new Set(SESSION_FORK_MODES);
const processSessionActionSet = new Set(PROCESS_SESSION_ACTIONS);
const processSessionRequiresCommandSet = new Set(
  manifest.processSession.requiresCommand,
);
const remoteEnvironmentRequiresEnvironmentSet = new Set(
  manifest.remoteEnvironment.requiresEnvironment,
);
const remoteEnvironmentRequiresEnvironmentIdSet = new Set(
  manifest.remoteEnvironment.requiresEnvironmentId,
);

export function isRuntimeSettingsAction(value) {
  return typeof value === "string" && runtimeSettingsActionSet.has(value);
}

export function isRemoteEnvironmentAction(value) {
  return typeof value === "string" && remoteEnvironmentActionSet.has(value);
}

export function isSessionForkMode(value) {
  return typeof value === "string" && sessionForkModeSet.has(value);
}

export function isProcessSessionAction(value) {
  return typeof value === "string" && processSessionActionSet.has(value);
}

export function normalizeRuntimeSettingsCommand(value) {
  const command = isRecord(value) && value.type === RUNTIME_SETTINGS_COMMAND_TYPE
    ? value
    : null;
  if (!command) return null;
  const action = stringOrNull(command.action);
  if (!isRuntimeSettingsAction(action)) {
    throw new Error("Lilia runtime settings command missing a valid action");
  }
  return { action };
}

export function createRuntimeSettingsCommand(action) {
  const normalized = normalizeRuntimeSettingsCommand({
    type: RUNTIME_SETTINGS_COMMAND_TYPE,
    action,
  });
  if (!normalized) throw new Error("Lilia runtime settings command missing a valid action");
  return {
    type: RUNTIME_SETTINGS_COMMAND_TYPE,
    action: normalized.action,
  };
}

export function normalizeRemoteEnvironmentCommand(value) {
  const command = isRecord(value) && value.type === REMOTE_ENVIRONMENT_COMMAND_TYPE
    ? value
    : null;
  if (!command) return null;
  const action = stringOrNull(command.action);
  if (!isRemoteEnvironmentAction(action)) {
    throw new Error("Lilia remote environment command missing a valid action");
  }
  const environmentId = stringOrNull(command.environmentId)?.trim() || "";
  const environment = jsonObjectOrNull(command.environment);
  if (remoteEnvironmentRequiresEnvironmentSet.has(action) && !environment) {
    throw new Error("Lilia remote environment add requires environment");
  }
  if (remoteEnvironmentRequiresEnvironmentIdSet.has(action) && !environmentId) {
    throw new Error("Lilia remote environment select requires environmentId");
  }
  if (action === "add" && environmentId && environment && !environment.id) {
    environment.id = environmentId;
  }
  return {
    action,
    environmentId,
    environment,
  };
}

export function createRemoteEnvironmentCommand(action, options = {}) {
  const normalized = normalizeRemoteEnvironmentCommand({
    type: REMOTE_ENVIRONMENT_COMMAND_TYPE,
    action,
    ...options,
  });
  if (!normalized) throw new Error("Lilia remote environment command missing a valid action");
  const command = {
    type: REMOTE_ENVIRONMENT_COMMAND_TYPE,
    action: normalized.action,
  };
  if (normalized.environmentId) command.environmentId = normalized.environmentId;
  if (normalized.environment) command.environment = normalized.environment;
  return command;
}

export function normalizeSandboxDiagnosticsCommand(value) {
  const command = isRecord(value) && value.type === SANDBOX_DIAGNOSTICS_COMMAND_TYPE
    ? value
    : null;
  if (!command) return null;
  return {
    includeDetails: command.includeDetails === true,
  };
}

export function createSandboxDiagnosticsCommand(options = {}) {
  const normalized = normalizeSandboxDiagnosticsCommand({
    type: SANDBOX_DIAGNOSTICS_COMMAND_TYPE,
    includeDetails: options.includeDetails,
  });
  if (!normalized) throw new Error("Lilia sandbox diagnostics command missing");
  return {
    type: SANDBOX_DIAGNOSTICS_COMMAND_TYPE,
    includeDetails: normalized.includeDetails,
  };
}

export function normalizeSessionForkCommand(value) {
  const command = isRecord(value) && value.type === SESSION_FORK_COMMAND_TYPE
    ? value
    : null;
  if (!command) return null;
  const sourceTurnId = stringOrNull(command.sourceTurnId)?.trim() || "";
  const mode = isSessionForkMode(command.mode) ? command.mode : DEFAULT_SESSION_FORK_MODE;
  return {
    excludeTurns: typeof command.excludeTurns === "boolean"
      ? command.excludeTurns
      : DEFAULT_SESSION_FORK_EXCLUDE_TURNS,
    sourceTurnId,
    mode,
  };
}

export function createSessionForkCommand(options = {}) {
  const normalized = normalizeSessionForkCommand({
    type: SESSION_FORK_COMMAND_TYPE,
    ...options,
  });
  if (!normalized) throw new Error("Lilia session fork command missing");
  return {
    type: SESSION_FORK_COMMAND_TYPE,
    excludeTurns: normalized.excludeTurns,
    ...(normalized.sourceTurnId ? { sourceTurnId: normalized.sourceTurnId } : {}),
    mode: normalized.mode,
  };
}

export function normalizeProcessSessionCommand(value) {
  const command = isRecord(value) && value.type === PROCESS_SESSION_COMMAND_TYPE
    ? value
    : null;
  if (!command) return null;
  const action = stringOrNull(command.action);
  if (!isProcessSessionAction(action)) {
    throw new Error("Lilia process session command missing a valid action");
  }
  const shellCommand = stringOrNull(command.command)?.trim() || "";
  if (processSessionRequiresCommandSet.has(action) && !shellCommand) {
    throw new Error("Lilia process session spawn requires command");
  }
  const rows = numberOrNull(command.rows);
  const cols = numberOrNull(command.cols);
  return {
    action,
    processId: stringOrNull(command.processId)?.trim() || "",
    command: shellCommand,
    cwd: stringOrNull(command.cwd)?.trim() || "",
    stdin: stringOrNull(command.stdin) || "",
    rows: rows && rows > 0 ? Math.trunc(rows) : null,
    cols: cols && cols > 0 ? Math.trunc(cols) : null,
    env: stringRecordOrNull(command.env),
    tty: command.tty === true,
    permissionProfile: stringOrNull(command.permissionProfile)?.trim() || "",
  };
}

export function createProcessSessionCommand(action, options = {}) {
  const normalized = normalizeProcessSessionCommand({
    type: PROCESS_SESSION_COMMAND_TYPE,
    action,
    ...options,
  });
  if (!normalized) throw new Error("Lilia process session command missing a valid action");
  const command = {
    type: PROCESS_SESSION_COMMAND_TYPE,
    action: normalized.action,
  };
  if (normalized.processId) command.processId = normalized.processId;
  if (normalized.command) command.command = normalized.command;
  if (normalized.cwd) command.cwd = normalized.cwd;
  if (normalized.stdin) command.stdin = normalized.stdin;
  if (normalized.rows !== null) command.rows = normalized.rows;
  if (normalized.cols !== null) command.cols = normalized.cols;
  if (normalized.env) command.env = normalized.env;
  if (normalized.tty) command.tty = true;
  if (normalized.permissionProfile) command.permissionProfile = normalized.permissionProfile;
  return command;
}

function numberOrNull(value) {
  const number = typeof value === "number" ? value : Number(value);
  return Number.isFinite(number) ? number : null;
}

function stringRecordOrNull(value) {
  if (!isRecord(value)) return null;
  const result = {};
  for (const [key, entry] of Object.entries(value)) {
    const normalized = stringOrNull(entry);
    if (typeof key === "string" && key && normalized !== null) {
      result[key] = normalized;
    }
  }
  return Object.keys(result).length ? result : null;
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function jsonObjectOrNull(value) {
  return isRecord(value) ? { ...value } : null;
}

function stringOrNull(value) {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return null;
}

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  for (const child of Object.values(value)) {
    deepFreeze(child);
  }
  return Object.freeze(value);
}
