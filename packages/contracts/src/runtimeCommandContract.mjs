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

const runtimeSettingsActionSet = new Set(RUNTIME_SETTINGS_ACTIONS);
const remoteEnvironmentActionSet = new Set(REMOTE_ENVIRONMENT_ACTIONS);
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
  return {
    excludeTurns: typeof command.excludeTurns === "boolean"
      ? command.excludeTurns
      : DEFAULT_SESSION_FORK_EXCLUDE_TURNS,
  };
}

export function createSessionForkCommand(options = {}) {
  return {
    type: SESSION_FORK_COMMAND_TYPE,
    excludeTurns: typeof options.excludeTurns === "boolean"
      ? options.excludeTurns
      : DEFAULT_SESSION_FORK_EXCLUDE_TURNS,
  };
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
