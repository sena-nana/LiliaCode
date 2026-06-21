import type {
  ChatBackendKind,
  ReasoningEffort,
} from "./chatBackendsContract.mjs";

export type ModelTier = "light" | "normal" | "deep";
export type ModelSelectionContextScale = "small" | "medium" | "large";

export interface ModelSelectionContextThresholds {
  contextUsagePercent: number;
  promptLength: number;
  attachmentCount: number;
  conversationReferenceCount: number;
  directoryFileCount?: number;
  directoryTotalSize?: number;
}

export const MODEL_SELECTION_TIERS: readonly ["light", "normal", "deep"];
export const MODEL_SELECTION_CONTEXT_SCALES: readonly ["medium", "large"];

export const AUTO_MODEL_BY_BACKEND_AND_TIER: Readonly<
  Record<ChatBackendKind, Readonly<Record<ModelTier, string>>>
>;
export const AUTO_REASONING_EFFORT_BY_TIER: Readonly<
  Record<ModelTier, ReasoningEffort>
>;
export const AUTO_WORKFLOW_TYPES_BY_TIER: Readonly<
  Record<ModelTier, readonly string[]>
>;
export const AUTO_RUNTIME_COMMAND_TYPES_BY_TIER: Readonly<
  Record<ModelTier, readonly string[]>
>;
export const AUTO_RUNTIME_COMMAND_SIGNAL_LABELS: Readonly<Record<string, string>>;
export const AUTO_CONTEXT_THRESHOLDS: Readonly<
  Record<Exclude<ModelSelectionContextScale, "small">, ModelSelectionContextThresholds>
>;

export function autoModelForBackendTier(
  backend: ChatBackendKind,
  tier: ModelTier,
): string;

export function autoReasoningEffortForTier(tier: ModelTier): ReasoningEffort;

export function autoTierForWorkflowType(
  value: string | null | undefined,
): ModelTier | null;

export function autoTierForRuntimeCommandType(
  value: string | null | undefined,
): ModelTier | null;

export function autoRuntimeCommandSignalLabel(
  value: string | null | undefined,
): string | null;

export function autoContextThresholdsForScale(
  scale: Exclude<ModelSelectionContextScale, "small">,
): ModelSelectionContextThresholds;
