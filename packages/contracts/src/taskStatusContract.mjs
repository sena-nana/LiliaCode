import taskStatusManifestJson from "./task-statuses.json" with { type: "json" };

export const TASK_STATUS_CONTRACT = deepFreeze(taskStatusManifestJson);

export const TASK_STATUSES = TASK_STATUS_CONTRACT.taskStatuses;
export const TASK_STATUS_LABELS = TASK_STATUS_CONTRACT.taskStatusLabels;
export const TASK_STATUS_STATE_LABELS =
  TASK_STATUS_CONTRACT.taskStatusStateLabels;
export const PROJECT_DASHBOARD_STATUS_ORDER =
  TASK_STATUS_CONTRACT.projectDashboardStatusOrder;
export const PROJECT_DASHBOARD_ACTIVE_STATUSES =
  TASK_STATUS_CONTRACT.projectDashboardActiveStatuses;
export const PROJECT_DASHBOARD_BLOCKED_STATUSES =
  TASK_STATUS_CONTRACT.projectDashboardBlockedStatuses;
export const PROJECT_ROADMAP_STATUS_ORDER =
  TASK_STATUS_CONTRACT.projectRoadmapStatusOrder;
export const MILESTONE_STATUSES = TASK_STATUS_CONTRACT.milestoneStatuses;
export const DEFAULT_MILESTONE_STATUS =
  TASK_STATUS_CONTRACT.defaultMilestoneStatus;
export const MILESTONE_STATUS_LABELS =
  TASK_STATUS_CONTRACT.milestoneStatusLabels;
export const MEMORY_SCOPES = TASK_STATUS_CONTRACT.memoryScopes;
export const DEFAULT_MEMORY_SCOPE = TASK_STATUS_CONTRACT.defaultMemoryScope;
export const MEMORY_SCOPE_TITLES = TASK_STATUS_CONTRACT.memoryScopeTitles;
export const MEMORY_SCOPE_EMPTY_LABELS =
  TASK_STATUS_CONTRACT.memoryScopeEmptyLabels;
export const DEFAULT_MEMORY_SETTINGS =
  TASK_STATUS_CONTRACT.defaultMemorySettings;

const taskStatusSet = new Set(TASK_STATUSES);
const milestoneStatusSet = new Set(MILESTONE_STATUSES);
const memoryScopeSet = new Set(MEMORY_SCOPES);

export function isTaskStatus(value) {
  return typeof value === "string" && taskStatusSet.has(value);
}

export function taskStatusLabel(status, variant = "compact") {
  return variant === "state"
    ? TASK_STATUS_STATE_LABELS[status]
    : TASK_STATUS_LABELS[status];
}

export function isMilestoneStatus(value) {
  return typeof value === "string" && milestoneStatusSet.has(value);
}

export function milestoneStatusLabel(status) {
  return MILESTONE_STATUS_LABELS[status];
}

export function normalizeMilestoneStatus(value, fallback = DEFAULT_MILESTONE_STATUS) {
  return isMilestoneStatus(value) ? value : fallback;
}

export function isMemoryScope(value) {
  return typeof value === "string" && memoryScopeSet.has(value);
}

export function memoryScopeLabel(scope) {
  return MEMORY_SCOPE_TITLES[scope];
}

export function memoryScopeEmptyLabel(scope) {
  return MEMORY_SCOPE_EMPTY_LABELS[scope];
}

export function normalizeMemoryScope(value, fallback = DEFAULT_MEMORY_SCOPE) {
  return isMemoryScope(value) ? value : fallback;
}

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  for (const child of Object.values(value)) {
    deepFreeze(child);
  }
  return Object.freeze(value);
}
