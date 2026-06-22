import architectureContract from "./architecture-contract.json" with { type: "json" };

const manifest = deepFreeze(architectureContract);

export const PROJECT_ARCHITECTURE_CONTRACT = manifest;
export const PROJECT_ARCHITECTURE_PERMISSIONS = manifest.projectArchitecturePermissions;
export const PROJECT_ARCHITECTURE_DEFAULT_PERMISSION = manifest.projectArchitectureDefaultPermission;
export const PROJECT_ARCHITECTURE_ROLLBACK_PERMISSION = manifest.projectArchitectureRollbackPermission;
export const PROJECT_ARCHITECTURE_RUNTIME_PERMISSION_MAP = manifest.projectArchitectureRuntimePermissionMap;
export const PROJECT_ARCHITECTURE_CHANGE_STATUSES = manifest.projectArchitectureChangeStatuses;
export const PROJECT_ARCHITECTURE_DEFAULT_CHANGE_STATUS = manifest.projectArchitectureDefaultChangeStatus;
export const PROJECT_ARCHITECTURE_DEFAULT_INTERACTION_STATUS = manifest.projectArchitectureDefaultInteractionStatus;
export const PROJECT_ARCHITECTURE_READONLY_INTERACTION_STATUS = manifest.projectArchitectureReadonlyInteractionStatus;
export const PROJECT_ARCHITECTURE_APPLIED_INTERACTION_STATUS = manifest.projectArchitectureAppliedInteractionStatus;
export const PROJECT_ARCHITECTURE_DEFAULT_NODE_TYPE = manifest.projectArchitectureDefaultNodeType;
export const PROJECT_ARCHITECTURE_DEFAULT_EDGE_TYPE = manifest.projectArchitectureDefaultEdgeType;
export const PROJECT_ARCHITECTURE_GET_COMMAND = manifest.commands.get;
export const PROJECT_ARCHITECTURE_LIST_CHANGES_COMMAND = manifest.commands.listChanges;
export const PROJECT_ARCHITECTURE_APPLY_COMMAND = manifest.commands.apply;
export const PROJECT_ARCHITECTURE_REJECT_COMMAND = manifest.commands.reject;
export const PROJECT_ARCHITECTURE_ROLLBACK_COMMAND = manifest.commands.rollback;
export const ARCHITECTURE_TOOL_NAMES = manifest.architectureToolNames;
export const ARCHITECTURE_TOOL_NAME = ARCHITECTURE_TOOL_NAMES[0];
export const ARCHITECTURE_CLAUDE_TOOL_NAME = ARCHITECTURE_TOOL_NAMES[1];
export const ARCHITECTURE_MCP_TOOL_NAME = ARCHITECTURE_TOOL_NAMES[2];
export const UPDATE_PROJECT_ARCHITECTURE_INPUT_SCHEMA = manifest.updateProjectArchitectureInputSchema;

const permissionSet = new Set(PROJECT_ARCHITECTURE_PERMISSIONS);
const changeStatusSet = new Set(PROJECT_ARCHITECTURE_CHANGE_STATUSES);
const architectureToolNameSet = new Set(ARCHITECTURE_TOOL_NAMES);

export function isProjectArchitecturePermission(value) {
  return typeof value === "string" && permissionSet.has(value);
}

export function normalizeProjectArchitecturePermission(
  value,
  fallback = PROJECT_ARCHITECTURE_DEFAULT_PERMISSION,
) {
  return isProjectArchitecturePermission(value) ? value : fallback;
}

export function projectArchitecturePermissionFromRuntimePermission(value) {
  return typeof value === "string"
    ? PROJECT_ARCHITECTURE_RUNTIME_PERMISSION_MAP[value] ?? PROJECT_ARCHITECTURE_DEFAULT_PERMISSION
    : PROJECT_ARCHITECTURE_DEFAULT_PERMISSION;
}

export function isProjectArchitectureChangeStatus(value) {
  return typeof value === "string" && changeStatusSet.has(value);
}

export function normalizeProjectArchitectureChangeStatus(
  value,
  fallback = PROJECT_ARCHITECTURE_DEFAULT_CHANGE_STATUS,
) {
  return isProjectArchitectureChangeStatus(value) ? value : fallback;
}

export function isLiliaArchitectureTool(toolName) {
  return architectureToolNameSet.has(String(toolName || ""));
}

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  for (const child of Object.values(value)) {
    deepFreeze(child);
  }
  return Object.freeze(value);
}
