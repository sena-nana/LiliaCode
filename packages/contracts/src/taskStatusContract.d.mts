export type TaskStatus =
  | "draft"
  | "waiting"
  | "running"
  | "blocked"
  | "done"
  | "cancelled";
export type MilestoneStatus = "upcoming" | "in-progress" | "done" | "abandoned";
export type MemoryScope = "user" | "project";

export interface MemorySettings {
  enabled: boolean;
  baselineInjectionEnabled: boolean;
  cooldownTurns: number;
}

export const TASK_STATUS_CONTRACT: Record<string, unknown>;
export const TASK_STATUSES: readonly [
  "draft",
  "waiting",
  "running",
  "blocked",
  "done",
  "cancelled",
];
export const TASK_STATUS_LABELS: Readonly<Record<TaskStatus, string>>;
export const TASK_STATUS_STATE_LABELS: Readonly<Record<TaskStatus, string>>;
export const PROJECT_DASHBOARD_STATUS_ORDER: readonly [
  "blocked",
  "running",
  "waiting",
  "draft",
  "done",
  "cancelled",
];
export const PROJECT_DASHBOARD_ACTIVE_STATUSES: readonly ["waiting", "running"];
export const PROJECT_DASHBOARD_BLOCKED_STATUSES: readonly ["blocked"];
export const PROJECT_ROADMAP_STATUS_ORDER: readonly [
  "running",
  "waiting",
  "blocked",
  "done",
  "cancelled",
];
export const MILESTONE_STATUSES: readonly [
  "upcoming",
  "in-progress",
  "done",
  "abandoned",
];
export const DEFAULT_MILESTONE_STATUS: MilestoneStatus;
export const MILESTONE_STATUS_LABELS: Readonly<Record<MilestoneStatus, string>>;
export const MEMORY_SCOPES: readonly ["user", "project"];
export const DEFAULT_MEMORY_SCOPE: MemoryScope;
export const MEMORY_SCOPE_TITLES: Readonly<Record<MemoryScope, string>>;
export const MEMORY_SCOPE_EMPTY_LABELS: Readonly<Record<MemoryScope, string>>;
export const DEFAULT_MEMORY_SETTINGS: MemorySettings;

export function isTaskStatus(value: unknown): value is TaskStatus;
export function taskStatusLabel(
  status: TaskStatus,
  variant?: "compact" | "state",
): string;
export function isMilestoneStatus(value: unknown): value is MilestoneStatus;
export function milestoneStatusLabel(status: MilestoneStatus): string;
export function normalizeMilestoneStatus(
  value: unknown,
  fallback?: MilestoneStatus,
): MilestoneStatus;
export function isMemoryScope(value: unknown): value is MemoryScope;
export function memoryScopeLabel(scope: MemoryScope): string;
export function memoryScopeEmptyLabel(scope: MemoryScope): string;
export function normalizeMemoryScope(
  value: unknown,
  fallback?: MemoryScope,
): MemoryScope;
