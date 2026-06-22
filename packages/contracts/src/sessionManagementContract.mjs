import sessionManagementContract from "./session-management-contract.json" with { type: "json" };

const manifest = deepFreeze(sessionManagementContract);

export const SESSION_MANAGEMENT_CONTRACT = manifest;
export const SESSION_MANAGEMENT_RUNTIME_COMMAND_TYPE = manifest.runtimeCommandType;
export const SESSION_MANAGEMENT_ACTIONS = manifest.actions;
export const DEFAULT_SESSION_MANAGEMENT_LIMIT = manifest.defaultLimit;
export const MAX_SESSION_MANAGEMENT_LIMIT = manifest.maxLimit;
export const DEFAULT_SESSION_MANAGEMENT_ARCHIVED = manifest.defaultArchived;

const actionSet = new Set(SESSION_MANAGEMENT_ACTIONS);
const requiresSessionIdSet = new Set(manifest.requiresSessionId);
const requiresTitleSet = new Set(manifest.requiresTitle);
const requiresTagFieldSet = new Set(manifest.requiresTagField);

export function isSessionManagementAction(value) {
  return typeof value === "string" && actionSet.has(value);
}

export function normalizeSessionManagementRuntimeCommand(value) {
  const command = isRecord(value) && value.type === SESSION_MANAGEMENT_RUNTIME_COMMAND_TYPE
    ? value
    : null;
  if (!command) return null;
  const action = stringOrNull(command.action);
  if (!isSessionManagementAction(action)) {
    throw new Error("Lilia session management runtime command missing a valid action");
  }
  const sessionId = stringOrNull(command.sessionId)?.trim() || "";
  const title = stringOrNull(command.title)?.trim() || "";
  const tag = hasOwn(command, "tag") ? stringOrNull(command.tag)?.trim() || null : null;
  if (requiresSessionIdSet.has(action) && !sessionId) {
    throw new Error("Lilia session management runtime command missing sessionId");
  }
  if (requiresTitleSet.has(action) && !title) {
    throw new Error("Lilia session management rename requires a non-empty title");
  }
  if (requiresTagFieldSet.has(action) && !hasOwn(command, "tag")) {
    throw new Error("Lilia session management tag requires tag");
  }
  return {
    action,
    sessionId,
    title,
    tag,
    archived: command.archived === true,
    limit: limitValue(command.limit),
    cursor: stringOrNull(command.cursor)?.trim() || null,
    searchTerm: stringOrNull(command.searchTerm)?.trim() || "",
    includeSystemMessages: command.includeSystemMessages === true,
  };
}

export function createSessionManagementRuntimeCommand(action, options = {}) {
  const normalized = normalizeSessionManagementRuntimeCommand({
    type: SESSION_MANAGEMENT_RUNTIME_COMMAND_TYPE,
    action,
    ...options,
  });
  if (!normalized) {
    throw new Error("Lilia session management runtime command missing a valid action");
  }
  const command = {
    type: SESSION_MANAGEMENT_RUNTIME_COMMAND_TYPE,
    action: normalized.action,
  };
  if (normalized.sessionId) command.sessionId = normalized.sessionId;
  if (normalized.title) command.title = normalized.title;
  if (hasOwn(options, "tag")) command.tag = normalized.tag;
  if (hasOwn(options, "archived")) command.archived = normalized.archived;
  if (hasOwn(options, "limit")) command.limit = normalized.limit;
  if (normalized.cursor) command.cursor = normalized.cursor;
  if (normalized.searchTerm) command.searchTerm = normalized.searchTerm;
  if (normalized.includeSystemMessages) command.includeSystemMessages = true;
  return command;
}

function limitValue(
  value,
  fallback = DEFAULT_SESSION_MANAGEMENT_LIMIT,
  max = MAX_SESSION_MANAGEMENT_LIMIT,
) {
  const number = Number(value);
  if (!Number.isFinite(number)) return fallback;
  return Math.max(1, Math.min(max, Math.trunc(number)));
}

function hasOwn(obj, key) {
  return Object.prototype.hasOwnProperty.call(obj, key);
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
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
