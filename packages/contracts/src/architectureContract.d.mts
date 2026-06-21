export type ProjectArchitecturePermission = "ask" | "full" | "readonly";
export type ProjectArchitectureChangeStatus =
  | "proposed"
  | "pending"
  | "applied"
  | "rejected"
  | "rolled_back";
export type ArchitectureToolName =
  | "UpdateProjectArchitecture"
  | "update_project_architecture"
  | "mcp__lilia__update_project_architecture";

export const PROJECT_ARCHITECTURE_CONTRACT: {
  projectArchitecturePermissions: readonly ProjectArchitecturePermission[];
  projectArchitectureDefaultPermission: ProjectArchitecturePermission;
  projectArchitectureRollbackPermission: ProjectArchitecturePermission;
  projectArchitectureRuntimePermissionMap: Readonly<Record<string, ProjectArchitecturePermission>>;
  projectArchitectureChangeStatuses: readonly ProjectArchitectureChangeStatus[];
  projectArchitectureDefaultChangeStatus: ProjectArchitectureChangeStatus;
  projectArchitectureDefaultInteractionStatus: "pending" | "proposed" | "applied";
  projectArchitectureReadonlyInteractionStatus: "proposed";
  projectArchitectureAppliedInteractionStatus: "applied";
  projectArchitectureDefaultNodeType: "module";
  projectArchitectureDefaultEdgeType: "depends_on";
  commands: {
    get: "project_architecture_get";
    listChanges: "project_architecture_list_changes";
    apply: "project_architecture_apply";
    reject: "project_architecture_reject";
    rollback: "project_architecture_rollback";
  };
  architectureToolNames: readonly ArchitectureToolName[];
  updateProjectArchitectureInputSchema: Record<string, unknown>;
};

export const PROJECT_ARCHITECTURE_PERMISSIONS: readonly ProjectArchitecturePermission[];
export const PROJECT_ARCHITECTURE_DEFAULT_PERMISSION: ProjectArchitecturePermission;
export const PROJECT_ARCHITECTURE_ROLLBACK_PERMISSION: ProjectArchitecturePermission;
export const PROJECT_ARCHITECTURE_RUNTIME_PERMISSION_MAP: Readonly<Record<string, ProjectArchitecturePermission>>;
export const PROJECT_ARCHITECTURE_CHANGE_STATUSES: readonly ProjectArchitectureChangeStatus[];
export const PROJECT_ARCHITECTURE_DEFAULT_CHANGE_STATUS: ProjectArchitectureChangeStatus;
export const PROJECT_ARCHITECTURE_DEFAULT_INTERACTION_STATUS: "pending" | "proposed" | "applied";
export const PROJECT_ARCHITECTURE_READONLY_INTERACTION_STATUS: "proposed";
export const PROJECT_ARCHITECTURE_APPLIED_INTERACTION_STATUS: "applied";
export const PROJECT_ARCHITECTURE_DEFAULT_NODE_TYPE: "module";
export const PROJECT_ARCHITECTURE_DEFAULT_EDGE_TYPE: "depends_on";
export const PROJECT_ARCHITECTURE_GET_COMMAND: "project_architecture_get";
export const PROJECT_ARCHITECTURE_LIST_CHANGES_COMMAND: "project_architecture_list_changes";
export const PROJECT_ARCHITECTURE_APPLY_COMMAND: "project_architecture_apply";
export const PROJECT_ARCHITECTURE_REJECT_COMMAND: "project_architecture_reject";
export const PROJECT_ARCHITECTURE_ROLLBACK_COMMAND: "project_architecture_rollback";
export const ARCHITECTURE_TOOL_NAMES: readonly ArchitectureToolName[];
export const ARCHITECTURE_TOOL_NAME: "UpdateProjectArchitecture";
export const ARCHITECTURE_CLAUDE_TOOL_NAME: "update_project_architecture";
export const ARCHITECTURE_MCP_TOOL_NAME: "mcp__lilia__update_project_architecture";
export const UPDATE_PROJECT_ARCHITECTURE_INPUT_SCHEMA: Record<string, unknown>;

export function isProjectArchitecturePermission(value: unknown): value is ProjectArchitecturePermission;

export function normalizeProjectArchitecturePermission(
  value: unknown,
  fallback?: ProjectArchitecturePermission,
): ProjectArchitecturePermission;

export function projectArchitecturePermissionFromRuntimePermission(
  value: unknown,
): ProjectArchitecturePermission;

export function isProjectArchitectureChangeStatus(
  value: unknown,
): value is ProjectArchitectureChangeStatus;

export function normalizeProjectArchitectureChangeStatus(
  value: unknown,
  fallback?: ProjectArchitectureChangeStatus,
): ProjectArchitectureChangeStatus;

export function isLiliaArchitectureTool(toolName: unknown): toolName is ArchitectureToolName;
