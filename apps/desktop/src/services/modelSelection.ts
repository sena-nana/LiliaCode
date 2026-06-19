import type {
  ChatAttachment,
  ChatBackendKind,
  ChatComposerState,
  ChatContextUsage,
  ChatConversationReference,
  ChatModelOption,
  ChatRuntimeCommand,
  ChatWorkflow,
  ModelSelectionExplanation,
  ProviderRuntimeOptions,
  ReasoningEffort,
} from "@lilia/contracts";

export const REASONING_EFFORTS: ReasoningEffort[] = ["low", "medium", "high", "xhigh", "max"];

type ContextScale = "small" | "medium" | "large";
type ModelTier = "light" | "normal" | "deep";
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
}

export interface ModelSelectionResult {
  composer: ChatComposerState;
  runtimeOptions: ProviderRuntimeOptions;
  explanation: ModelSelectionExplanation;
}

const AUTO_MODELS: Record<ChatBackendKind, Record<ModelTier, string>> = {
  codex: {
    light: "gpt-5.4-mini",
    normal: "gpt-5.4",
    deep: "gpt-5.5",
  },
  claude: {
    light: "claude-haiku-4-5",
    normal: "claude-sonnet-4-6",
    deep: "claude-opus-4-7",
  },
};

const AUTO_EFFORTS: Record<ModelTier, ReasoningEffort> = {
  light: "low",
  normal: "medium",
  deep: "high",
};

const LIGHT_WORKFLOWS = new Set([
  "lilia_compact",
  "lilia_background_terminals_clean",
  "lilia_config_diagnostics",
  "lilia_memory_mode",
  "lilia_memory_reset",
]);

const DEEP_WORKFLOWS = new Set([
  "lilia_review",
  "lilia_fix_suggestion",
  "lilia_batch_apply",
]);

function normalizeEffort(value: unknown): ReasoningEffort | null {
  return typeof value === "string" && REASONING_EFFORTS.includes(value as ReasoningEffort)
    ? value as ReasoningEffort
    : null;
}

function normalizeEffortForBackend(
  backend: ChatBackendKind,
  value: ReasoningEffort | null,
  signals: string[],
): ReasoningEffort | null {
  if (!value) return null;
  if (backend === "codex" && value === "max") {
    signals.push("Codex 不支持 max，已降级为 xhigh");
    return "xhigh";
  }
  return value;
}

function runtimeProviderModel(
  backend: ChatBackendKind,
  runtimeOptions: ProviderRuntimeOptions | null | undefined,
): string | null {
  const commonModel = runtimeOptions?.common?.model?.trim();
  if (commonModel) return commonModel;
  if (backend === "codex") {
    const codexModel = runtimeOptions?.provider?.codex?.model?.trim();
    if (codexModel) return codexModel;
  }
  return null;
}

function runtimeProviderEffort(
  backend: ChatBackendKind,
  runtimeOptions: ProviderRuntimeOptions | null | undefined,
): ReasoningEffort | null {
  const provider = runtimeOptions?.provider;
  const providerEffort = backend === "codex"
    ? normalizeEffort(provider?.codex?.reasoningEffort)
    : normalizeEffort(provider?.claude?.reasoningEffort);
  return providerEffort ?? normalizeEffort(runtimeOptions?.common?.reasoningEffort);
}

function workflowType(workflow: ChatWorkflow | null | undefined): string | null {
  return typeof workflow?.type === "string" ? workflow.type : null;
}

function contextScale(input: ModelSelectionInput, signals: string[]): ContextScale {
  const promptLength = input.prompt.trim().length;
  const attachmentCount = input.attachments?.length ?? 0;
  const referenceCount = input.conversationReferences?.length ?? 0;
  const usagePercent = input.contextUsage?.backend === input.backend ? input.contextUsage.usedPercent : null;
  const hasLargeDirectory = (input.attachments ?? []).some((attachment) => {
    const directory = attachment.directory;
    return Boolean(directory && (
      directory.truncated ||
      directory.fileCount >= 200 ||
      directory.totalSize >= 20 * 1024 * 1024
    ));
  });
  if (
    (typeof usagePercent === "number" && usagePercent >= 70) ||
    promptLength >= 8000 ||
    attachmentCount >= 6 ||
    hasLargeDirectory ||
    referenceCount >= 3
  ) {
    signals.push("上下文规模 large");
    return "large";
  }
  if (
    (typeof usagePercent === "number" && usagePercent >= 35) ||
    promptLength >= 2000 ||
    attachmentCount >= 2 ||
    referenceCount >= 1
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
  if (type && DEEP_WORKFLOWS.has(type)) {
    signals.push(`工作流 ${type}`);
    return "deep";
  }
  if (type && LIGHT_WORKFLOWS.has(type)) {
    signals.push(`轻量工作流 ${type}`);
    return "light";
  }
  if (input.runtimeCommand?.type === "runtime_settings") {
    signals.push("运行时诊断/设置");
    return "light";
  }
  if (input.runtimeCommand?.type === "remote_environment") {
    signals.push("远程环境管理");
    return "light";
  }
  if (input.runtimeCommand?.type === "session_management") {
    signals.push("会话管理");
    return "light";
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
): string {
  const desired = AUTO_MODELS[backend][tier];
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
  if (modelExists(modelOptions, model)) return model;
  if (backend === "claude" && model.startsWith("claude-")) return model;
  if (backend === "codex" && (model.startsWith("gpt-") || model.startsWith("o"))) return model;
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

function mergeRuntimeOptions(
  backend: ChatBackendKind,
  runtimeOptions: ProviderRuntimeOptions | null | undefined,
  model: string,
  effort: ReasoningEffort | null,
  explanation: ModelSelectionExplanation,
): ProviderRuntimeOptions {
  const next: ProviderRuntimeOptions = {
    ...(runtimeOptions ?? {}),
    common: {
      ...(runtimeOptions?.common ?? {}),
      model,
      reasoningEffort: effort ?? undefined,
      modelSelection: explanation,
    },
    provider: {
      ...(runtimeOptions?.provider ?? {}),
    },
  };
  if (backend === "codex") {
    const codexEffort = effort === "max" ? "xhigh" : effort;
    next.provider = {
      ...next.provider,
      codex: {
        ...(runtimeOptions?.provider?.codex ?? {}),
        model,
        reasoningEffort: codexEffort ?? undefined,
      },
    };
  } else {
    next.provider = {
      ...next.provider,
      claude: {
        ...(runtimeOptions?.provider?.claude ?? {}),
        reasoningEffort: effort ?? undefined,
        thinking: runtimeOptions?.provider?.claude?.thinking ?? (effort ? { type: "adaptive" } : undefined),
      },
    };
  }
  return next;
}

export function selectModelForTurn(input: ModelSelectionInput): ModelSelectionResult {
  const signals: string[] = [];
  const backend = input.backend;
  const tier = selectAutoTier(input, signals);
  const autoModel = pickAvailableModel(backend, tier, input.modelOptions, signals);
  const autoEffort = AUTO_EFFORTS[tier];
  const runtimeModel = runtimeProviderModel(backend, input.runtimeOptions);
  const runtimeEffort = runtimeProviderEffort(backend, input.runtimeOptions);
  const manualMode = input.composer.modelSelectionMode === "manual";
  const manualModel = manualMode && input.composer.model ? input.composer.model : null;
  const manualEffort = manualMode ? normalizeEffort(input.composer.reasoningEffort) : null;
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
    runtimeOptions: mergeRuntimeOptions(backend, input.runtimeOptions, selectedModel, selectedEffort, explanation),
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
