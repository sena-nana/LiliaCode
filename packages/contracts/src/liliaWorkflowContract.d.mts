export type LiliaGoalAction = "set" | "refresh" | "clear";
export type LiliaGoalStatus =
  | "active"
  | "paused"
  | "blocked"
  | "usageLimited"
  | "budgetLimited"
  | "complete";
export type LiliaMemoryMode = "enabled" | "disabled";
export type LiliaReviewTargetType = "uncommittedChanges" | "baseBranch" | "commit";
export type LiliaReviewDelivery = "inline" | "detached";
export type LiliaFixSuggestionMode = "suggest" | "apply";
export type LiliaBatchApplySourceKind = "review" | "fix_suggestion";
export type LiliaTaskWorkflowKind =
  | "generalTask"
  | "review"
  | "bugLocalization"
  | "frontend"
  | "refactor"
  | "testAndVerification"
  | "docsAndPrompt"
  | "gitAndRelease"
  | "architectureAndMemory";
export type LiliaQueryWorkflowType =
  | "lilia_review"
  | "lilia_fix_suggestion"
  | "lilia_batch_apply"
  | "lilia_task_workflow"
  | "lilia_compact";

export type LiliaReviewTarget =
  | { type: "uncommittedChanges" }
  | { type: "baseBranch"; branch: string }
  | { type: "commit"; sha: string };

export interface NormalizedLiliaGoalWorkflow {
  action: LiliaGoalAction;
  objective: string;
  status: LiliaGoalStatus;
  tokenBudget: number | null;
}

export interface NormalizedLiliaMemoryModeWorkflow {
  mode: LiliaMemoryMode;
}

export interface NormalizedLiliaConfigDiagnosticsWorkflow {
  includeLayers: boolean;
}

export interface NormalizedLiliaReviewWorkflow {
  target: LiliaReviewTarget;
  instructions: string;
  delivery: LiliaReviewDelivery;
}

export interface NormalizedLiliaTaskWorkflow {
  kind: LiliaTaskWorkflowKind;
  instructions: string;
}

export interface NormalizedLiliaFixSuggestionWorkflow {
  target: LiliaReviewTarget;
  instructions: string;
  mode: LiliaFixSuggestionMode;
}

export interface NormalizedLiliaBatchApplyWorkflow {
  sourceTurnId: string;
  sourceKind: LiliaBatchApplySourceKind;
  sourceSummary: string;
  instructions: string;
}

export interface CreateLiliaReviewWorkflowOptions {
  instructions?: string;
  delivery?: LiliaReviewDelivery;
}

export interface CreateLiliaTaskWorkflowOptions {
  instructions?: string;
}

export interface CreateLiliaFixSuggestionWorkflowOptions {
  instructions?: string;
  mode?: LiliaFixSuggestionMode;
}

export interface CreateLiliaBatchApplyWorkflowOptions {
  instructions?: string;
}

export interface CreateLiliaGoalWorkflowOptions {
  objective?: string;
  status?: LiliaGoalStatus;
  tokenBudget?: number | null;
}

export const LILIA_WORKFLOW_CONTRACT: {
  queryWorkflowTypes: readonly LiliaQueryWorkflowType[];
  taskWorkflow: {
    type: "lilia_task_workflow";
    kinds: readonly LiliaTaskWorkflowKind[];
  };
  review: {
    type: "lilia_review";
    targetTypes: readonly LiliaReviewTargetType[];
    deliveries: readonly LiliaReviewDelivery[];
    defaultDelivery: LiliaReviewDelivery;
  };
  fixSuggestion: {
    type: "lilia_fix_suggestion";
    modes: readonly LiliaFixSuggestionMode[];
    defaultMode: LiliaFixSuggestionMode;
  };
  batchApply: {
    type: "lilia_batch_apply";
    sourceKinds: readonly LiliaBatchApplySourceKind[];
  };
  goal: {
    type: "lilia_goal";
    actions: readonly LiliaGoalAction[];
    statuses: readonly LiliaGoalStatus[];
    defaultStatus: LiliaGoalStatus;
    requiresObjective: readonly LiliaGoalAction[];
  };
  memoryMode: {
    type: "lilia_memory_mode";
    modes: readonly LiliaMemoryMode[];
  };
  memoryReset: {
    type: "lilia_memory_reset";
  };
  compact: {
    type: "lilia_compact";
  };
  backgroundTerminalsClean: {
    type: "lilia_background_terminals_clean";
  };
  configDiagnostics: {
    type: "lilia_config_diagnostics";
    defaultIncludeLayers: boolean;
  };
  automation: {
    type: "automation";
  };
  slashCommand: {
    type: "slash_command";
  };
};

export const LILIA_QUERY_WORKFLOW_TYPES: readonly LiliaQueryWorkflowType[];
export const LILIA_TASK_WORKFLOW_TYPE: "lilia_task_workflow";
export const LILIA_TASK_WORKFLOW_KINDS: readonly LiliaTaskWorkflowKind[];
export const LILIA_REVIEW_WORKFLOW_TYPE: "lilia_review";
export const LILIA_REVIEW_TARGET_TYPES: readonly LiliaReviewTargetType[];
export const LILIA_REVIEW_DELIVERIES: readonly LiliaReviewDelivery[];
export const DEFAULT_LILIA_REVIEW_DELIVERY: LiliaReviewDelivery;
export const LILIA_FIX_SUGGESTION_WORKFLOW_TYPE: "lilia_fix_suggestion";
export const LILIA_FIX_SUGGESTION_MODES: readonly LiliaFixSuggestionMode[];
export const DEFAULT_LILIA_FIX_SUGGESTION_MODE: LiliaFixSuggestionMode;
export const LILIA_BATCH_APPLY_WORKFLOW_TYPE: "lilia_batch_apply";
export const LILIA_BATCH_APPLY_SOURCE_KINDS: readonly LiliaBatchApplySourceKind[];
export const LILIA_GOAL_WORKFLOW_TYPE: "lilia_goal";
export const LILIA_GOAL_ACTIONS: readonly LiliaGoalAction[];
export const LILIA_GOAL_STATUSES: readonly LiliaGoalStatus[];
export const DEFAULT_LILIA_GOAL_STATUS: LiliaGoalStatus;
export const LILIA_MEMORY_MODE_WORKFLOW_TYPE: "lilia_memory_mode";
export const LILIA_MEMORY_MODES: readonly LiliaMemoryMode[];
export const LILIA_MEMORY_RESET_WORKFLOW_TYPE: "lilia_memory_reset";
export const LILIA_COMPACT_WORKFLOW_TYPE: "lilia_compact";
export const LILIA_BACKGROUND_TERMINALS_CLEAN_WORKFLOW_TYPE: "lilia_background_terminals_clean";
export const LILIA_CONFIG_DIAGNOSTICS_WORKFLOW_TYPE: "lilia_config_diagnostics";
export const DEFAULT_LILIA_CONFIG_DIAGNOSTICS_INCLUDE_LAYERS: boolean;
export const AUTOMATION_WORKFLOW_TYPE: "automation";
export const CHAT_SLASH_COMMAND_WORKFLOW_TYPE: "slash_command";

export function isLiliaReviewTargetType(value: unknown): value is LiliaReviewTargetType;

export function isLiliaQueryWorkflowType(value: unknown): value is LiliaQueryWorkflowType;

export function isLiliaTaskWorkflowKind(value: unknown): value is LiliaTaskWorkflowKind;

export function isLiliaReviewDelivery(value: unknown): value is LiliaReviewDelivery;

export function isLiliaFixSuggestionMode(value: unknown): value is LiliaFixSuggestionMode;

export function isLiliaBatchApplySourceKind(value: unknown): value is LiliaBatchApplySourceKind;

export function isLiliaGoalAction(value: unknown): value is LiliaGoalAction;

export function isLiliaGoalStatus(value: unknown): value is LiliaGoalStatus;

export function isLiliaMemoryMode(value: unknown): value is LiliaMemoryMode;

export function normalizeLiliaGoalStatus(value: unknown): LiliaGoalStatus;

export function normalizeLiliaReviewTarget(value: unknown): LiliaReviewTarget | null;

export function normalizeLiliaReviewDelivery(value: unknown): LiliaReviewDelivery;

export function normalizeLiliaFixSuggestionMode(value: unknown): LiliaFixSuggestionMode;

export function normalizeLiliaReviewWorkflow(
  value: unknown,
): NormalizedLiliaReviewWorkflow | null;

export function normalizeLiliaTaskWorkflow(
  value: unknown,
): NormalizedLiliaTaskWorkflow | null;

export function normalizeLiliaFixSuggestionWorkflow(
  value: unknown,
): NormalizedLiliaFixSuggestionWorkflow | null;

export function normalizeLiliaBatchApplyWorkflow(
  value: unknown,
): NormalizedLiliaBatchApplyWorkflow | null;

export function createLiliaReviewWorkflow(
  target: LiliaReviewTarget,
  options?: CreateLiliaReviewWorkflowOptions,
): {
  type: "lilia_review";
  target: LiliaReviewTarget;
  instructions?: string;
  delivery?: LiliaReviewDelivery;
};

export function createLiliaTaskWorkflow(
  kind: LiliaTaskWorkflowKind,
  options?: CreateLiliaTaskWorkflowOptions,
): {
  type: "lilia_task_workflow";
  kind: LiliaTaskWorkflowKind;
  instructions?: string;
};

export function createLiliaFixSuggestionWorkflow(
  target: LiliaReviewTarget,
  options?: CreateLiliaFixSuggestionWorkflowOptions,
): {
  type: "lilia_fix_suggestion";
  target: LiliaReviewTarget;
  instructions?: string;
  mode?: LiliaFixSuggestionMode;
};

export function createLiliaBatchApplyWorkflow(
  input: Pick<{
    sourceTurnId: string;
    sourceKind: LiliaBatchApplySourceKind;
    sourceSummary: string;
  }, "sourceTurnId" | "sourceKind" | "sourceSummary">,
  options?: CreateLiliaBatchApplyWorkflowOptions,
): {
  type: "lilia_batch_apply";
  sourceTurnId: string;
  sourceKind: LiliaBatchApplySourceKind;
  sourceSummary: string;
  instructions?: string;
};

export function normalizeLiliaGoalWorkflow(
  value: unknown,
): NormalizedLiliaGoalWorkflow | null;

export function createLiliaGoalWorkflow(
  action: LiliaGoalAction,
  options?: CreateLiliaGoalWorkflowOptions,
): {
  type: "lilia_goal";
  action: LiliaGoalAction;
  objective?: string;
  status?: LiliaGoalStatus;
  tokenBudget?: number | null;
};

export function createLiliaCompactWorkflow(): {
  type: "lilia_compact";
};

export function normalizeLiliaMemoryModeWorkflow(
  value: unknown,
): NormalizedLiliaMemoryModeWorkflow | null;

export function isLiliaMemoryResetWorkflow(value: unknown): boolean;

export function isLiliaCompactWorkflow(value: unknown): boolean;

export function isLiliaBackgroundTerminalsCleanWorkflow(value: unknown): boolean;

export function normalizeLiliaConfigDiagnosticsWorkflow(
  value: unknown,
): NormalizedLiliaConfigDiagnosticsWorkflow | null;
