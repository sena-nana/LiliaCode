import {
  autoModelForBackendTier,
  autoRuntimeCommandSignalLabel,
  autoReasoningEffortForTier,
  autoTierForRuntimeCommandType,
  autoTierForWorkflowType,
  autoContextThresholdsForScale,
  chatBackendLabel,
  mergeModelSelectionRuntimeOptions,
  modelBelongsToBackend,
  normalizeReasoningEffortForBackend,
  normalizeReasoningEffort,
  runtimeOptionsModelForBackend,
  runtimeOptionsReasoningEffortForBackend,
  type ChatAttachment,
  type ChatBackendKind,
  type ChatComposerState,
  type ChatContextUsage,
  type ChatConversationReference,
  type ChatModelOption,
  type ChatRuntimeCommand,
  type ChatWorkflow,
  type ModelFeatureSettings,
  type ModelSelectionContextScale,
  type ModelSelectionExplanation,
  type ModelTier,
  type ProviderRuntimeOptions,
  type ReasoningEffort,
} from "@lilia/contracts";

type SelectionSource = ModelSelectionExplanation["source"];

export interface ModelSelectionInput {
  backend: ChatBackendKind;
  modelOptions: ChatModelOption[];
  composer: ChatComposerState;
  prompt: string;
  attachments?: ChatAttachment[];
  conversationReferences?: ChatConversationReference[];
  contextUsage?: ChatContextUsage | null;
  workflow?: ChatWorkflow | null;
  runtimeCommand?: ChatRuntimeCommand | null;
  runtimeOptions?: ProviderRuntimeOptions | null;
  modelFeatureSettings?: ModelFeatureSettings | null;
}

export interface ModelSelectionResult {
  composer: ChatComposerState;
  runtimeOptions: ProviderRuntimeOptions;
  explanation: ModelSelectionExplanation;
}

function normalizeEffortForBackend(
  backend: ChatBackendKind,
  value: ReasoningEffort | null,
  signals: string[],
): ReasoningEffort | null {
  if (!value) return null;
  const normalized = normalizeReasoningEffortForBackend(backend, value);
  if (normalized && normalized !== value) {
    signals.push(`${chatBackendLabel(backend)} 不支持 ${value}，已降级为 ${normalized}`);
  }
  return normalized;
}

function workflowType(workflow: ChatWorkflow | null | undefined): string | null {
  return typeof workflow?.type === "string" ? workflow.type : null;
}

function contextScale(input: ModelSelectionInput, signals: string[]): ModelSelectionContextScale {
  const promptLength = input.prompt.trim().length;
  const attachmentCount = input.attachments?.length ?? 0;
  const referenceCount = input.conversationReferences?.length ?? 0;
  const usagePercent = input.contextUsage?.backend === input.backend ? input.contextUsage.usedPercent : null;
  const largeThresholds = autoContextThresholdsForScale("large");
  const mediumThresholds = autoContextThresholdsForScale("medium");
  const hasLargeDirectory = (input.attachments ?? []).some((attachment) => {
    const directory = attachment.directory;
    return Boolean(directory && (
      directory.truncated ||
      (
        typeof largeThresholds.directoryFileCount === "number" &&
        directory.fileCount >= largeThresholds.directoryFileCount
      ) ||
      (
        typeof largeThresholds.directoryTotalSize === "number" &&
        directory.totalSize >= largeThresholds.directoryTotalSize
      )
    ));
  });
  if (
    (typeof usagePercent === "number" && usagePercent >= largeThresholds.contextUsagePercent) ||
    promptLength >= largeThresholds.promptLength ||
    attachmentCount >= largeThresholds.attachmentCount ||
    hasLargeDirectory ||
    referenceCount >= largeThresholds.conversationReferenceCount
  ) {
    signals.push("上下文规模 large");
    return "large";
  }
  if (
    (typeof usagePercent === "number" && usagePercent >= mediumThresholds.contextUsagePercent) ||
    promptLength >= mediumThresholds.promptLength ||
    attachmentCount >= mediumThresholds.attachmentCount ||
    referenceCount >= mediumThresholds.conversationReferenceCount
  ) {
    signals.push("上下文规模 medium");
    return "medium";
  }
  signals.push("上下文规模 small");
  return "small";
}

function selectAutoTier(input: ModelSelectionInput, signals: string[]): ModelTier {
  const type = workflowType(input.workflow);
  if (input.composer.planMode) {
    signals.push("计划模式");
    return "deep";
  }
  const workflowTier = autoTierForWorkflowType(type);
  if (workflowTier) {
    signals.push(
      workflowTier === "light" ? `轻量工作流 ${type}` : `工作流 ${type}`,
    );
    return workflowTier;
  }
  const runtimeCommandTier = autoTierForRuntimeCommandType(input.runtimeCommand?.type);
  if (runtimeCommandTier) {
    signals.push(autoRuntimeCommandSignalLabel(input.runtimeCommand?.type) ?? "运行时命令");
    return runtimeCommandTier;
  }
  const scale = contextScale(input, signals);
  if (scale === "large") return "deep";
  if (scale === "medium") return "normal";
  return "light";
}

function modelExists(modelOptions: ChatModelOption[], model: string): boolean {
  return modelOptions.some((option) => option.id === model);
}

function pickAvailableModel(
  backend: ChatBackendKind,
  tier: ModelTier,
  modelOptions: ChatModelOption[],
  signals: string[],
  modelFeatureSettings?: ModelFeatureSettings | null,
): string {
  const desired = modelFeatureSettings?.chat?.[tier]?.trim() || autoModelForBackendTier(backend, tier);
  if (modelExists(modelOptions, desired)) return desired;
  const fallback = modelOptions.find((option) => option.backend === backend)?.id ?? desired;
  if (fallback !== desired) signals.push(`模型 ${desired} 不可用，已使用 ${fallback}`);
  return fallback;
}

function validateModel(
  backend: ChatBackendKind,
  model: string,
  modelOptions: ChatModelOption[],
  fallbackTier: ModelTier,
  signals: string[],
): string {
  if (modelBelongsToBackend(backend, model)) return model;
  signals.push(`模型 ${model} 不属于当前后端，已回退自动档位`);
  return pickAvailableModel(backend, fallbackTier, modelOptions, signals);
}

function selectionSummary(
  source: SelectionSource,
  model: string,
  effort: ReasoningEffort | null,
  signals: string[],
): string {
  const prefix = source === "auto"
    ? "自动选择"
    : source === "manual"
      ? "手动覆盖"
      : "runtimeOptions 覆盖";
  const effortText = effort ? `，thinking ${effort}` : "";
  const signalText = signals.length ? `；${signals.join("；")}` : "";
  return `${prefix} ${model}${effortText}${signalText}`;
}

export function selectModelForTurn(input: ModelSelectionInput): ModelSelectionResult {
  const signals: string[] = [];
  const backend = input.backend;
  const tier = selectAutoTier(input, signals);
  const autoModel = pickAvailableModel(
    backend,
    tier,
    input.modelOptions,
    signals,
    input.modelFeatureSettings,
  );
  const autoEffort = autoReasoningEffortForTier(tier);
  const runtimeModel = runtimeOptionsModelForBackend(backend, input.runtimeOptions);
  const runtimeEffort = runtimeOptionsReasoningEffortForBackend(backend, input.runtimeOptions);
  const manualMode = input.composer.modelSelectionMode === "manual";
  const manualModel = manualMode && input.composer.model ? input.composer.model : null;
  const manualEffort = manualMode ? normalizeReasoningEffort(input.composer.reasoningEffort) : null;
  const source: SelectionSource =
    runtimeModel || runtimeEffort ? "runtimeOptions" : manualModel || manualEffort ? "manual" : "auto";
  const selectedModel = validateModel(
    backend,
    runtimeModel ?? manualModel ?? autoModel,
    input.modelOptions,
    tier,
    signals,
  );
  const selectedEffort = normalizeEffortForBackend(
    backend,
    runtimeEffort ?? manualEffort ?? autoEffort,
    signals,
  );
  if (source === "manual") signals.push("用户手动覆盖");
  if (source === "runtimeOptions") signals.push("runtimeOptions 显式覆盖");
  const explanation: ModelSelectionExplanation = {
    mode: manualMode ? "manual" : "auto",
    model: selectedModel,
    reasoningEffort: selectedEffort,
    source,
    signals,
    summary: selectionSummary(source, selectedModel, selectedEffort, signals),
  };
  const composer: ChatComposerState = {
    ...input.composer,
    backend,
    model: source === "auto" ? autoModel : selectedModel,
    modelSelectionMode: manualMode ? "manual" : "auto",
    reasoningEffort: manualMode ? selectedEffort : null,
  };
  return {
    composer,
    runtimeOptions: mergeModelSelectionRuntimeOptions(
      backend,
      input.runtimeOptions,
      selectedModel,
      selectedEffort,
      explanation,
    ),
    explanation,
  };
}

export function previewAutoModelSelection(input: Omit<ModelSelectionInput, "runtimeOptions">): ModelSelectionExplanation {
  return selectModelForTurn({
    ...input,
    composer: {
      ...input.composer,
      modelSelectionMode: "auto",
      reasoningEffort: null,
    },
    runtimeOptions: null,
  }).explanation;
}

