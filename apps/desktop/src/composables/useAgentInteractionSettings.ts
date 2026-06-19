import { computed, readonly, ref } from "vue";
import type {
  AgentInteractionSettings,
  CustomSubagentDefinition,
  CustomSubagentUpsertInput,
  PermissionMode,
} from "@lilia/contracts";
import {
  deleteCustomSubagent,
  getAgentInteractionSettings,
  listCustomSubagents,
  setAgentInteractionSettings,
  upsertCustomSubagent,
} from "../services/chat";

const DEFAULT_AGENT_INTERACTION_SETTINGS: AgentInteractionSettings = {
  nonInterruptMode: false,
  debug: false,
  permissionMode: "ask",
  codexProfile: {
    profile: "default",
    model: null,
    reasoningEffort: null,
    runtimeWorkspaceRoots: [],
    responsesApiClientMetadata: null,
    additionalContext: null,
    persistExtendedHistory: null,
    initialTurnsPage: null,
    excludeTurns: [],
  },
  subagentMode: {
    enabled: false,
    codex: {
      enabled: true,
    },
    claude: {
      enabled: true,
      forwardSubagentText: true,
      agentProgressSummaries: true,
    },
  },
  autoTurnDecision: {
    enabled: true,
    allowModelTier: true,
    allowReasoningEffort: true,
    allowPlanMode: true,
    allowGoalMode: true,
    allowSessionFork: true,
  },
};

const settings = ref<AgentInteractionSettings>({
  ...DEFAULT_AGENT_INTERACTION_SETTINGS,
});
const subagents = ref<CustomSubagentDefinition[]>([]);

let loadPromise: Promise<AgentInteractionSettings> | null = null;

export function uniqueTrimmedStrings(value: unknown): string[] {
  return Array.isArray(value)
    ? Array.from(new Set(
        value
          .filter((item): item is string => typeof item === "string")
          .map((item) => item.trim())
          .filter(Boolean),
      ))
    : [];
}

export function normalizeAgentInteractionSettings(
  input: Partial<AgentInteractionSettings> | null | undefined,
): AgentInteractionSettings {
  const codexProfile = input?.codexProfile;
  const subagentMode = input?.subagentMode;
  const claudeSubagentMode = subagentMode?.claude;
  return {
    nonInterruptMode: input?.nonInterruptMode === true,
    debug: input?.debug === true,
    permissionMode: normalizePermissionMode(input?.permissionMode),
    codexProfile: {
      profile: normalizeProfile(codexProfile?.profile),
      model: normalizeNullableText(codexProfile?.model),
      reasoningEffort: normalizeReasoningEffort(codexProfile?.reasoningEffort),
      runtimeWorkspaceRoots: uniqueTrimmedStrings(codexProfile?.runtimeWorkspaceRoots),
      responsesApiClientMetadata: normalizeJsonObject(codexProfile?.responsesApiClientMetadata),
      additionalContext: normalizeNullableText(codexProfile?.additionalContext),
      persistExtendedHistory: normalizeNullableBoolean(codexProfile?.persistExtendedHistory),
      initialTurnsPage: normalizeJsonObject(codexProfile?.initialTurnsPage),
      excludeTurns: uniqueTrimmedStrings(codexProfile?.excludeTurns),
    },
    subagentMode: {
      enabled: subagentMode?.enabled === true,
      codex: {
        enabled: subagentMode?.codex?.enabled !== false,
      },
      claude: {
        enabled: claudeSubagentMode?.enabled !== false,
        forwardSubagentText: claudeSubagentMode?.forwardSubagentText !== false,
        agentProgressSummaries: claudeSubagentMode?.agentProgressSummaries !== false,
      },
    },
    autoTurnDecision: normalizeAutoTurnDecisionSettings(input?.autoTurnDecision),
  };
}

function normalizeAutoTurnDecisionSettings(
  input: Partial<AgentInteractionSettings["autoTurnDecision"]> | null | undefined,
): AgentInteractionSettings["autoTurnDecision"] {
  return {
    enabled: input?.enabled !== false,
    allowModelTier: input?.allowModelTier !== false,
    allowReasoningEffort: input?.allowReasoningEffort !== false,
    allowPlanMode: input?.allowPlanMode !== false,
    allowGoalMode: input?.allowGoalMode !== false,
    allowSessionFork: input?.allowSessionFork !== false,
  };
}

function normalizeJsonObject(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? { ...(value as Record<string, unknown>) }
    : null;
}

function normalizeNullableBoolean(value: unknown): boolean | null {
  return typeof value === "boolean" ? value : null;
}

function normalizeNullableText(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeReasoningEffort(value: unknown): AgentInteractionSettings["codexProfile"]["reasoningEffort"] {
  return value === "low" || value === "medium" || value === "high" || value === "xhigh"
    ? value
    : null;
}

function normalizeProfile(value: unknown): AgentInteractionSettings["codexProfile"]["profile"] {
  return value === "fast" || value === "balanced" || value === "deep" ? value : "default";
}

export function normalizePermissionMode(value: unknown): PermissionMode {
  return value === "full" || value === "readonly" || value === "free" ? value : "ask";
}

function sameJsonValue(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function sameCodexProfile(a: unknown, b: unknown): boolean {
  return sameJsonValue(a, b);
}

export function sameSubagentMode(a: unknown, b: unknown): boolean {
  return sameJsonValue(a, b);
}

export async function loadAgentInteractionSettings(): Promise<AgentInteractionSettings> {
  if (!loadPromise) {
    loadPromise = getAgentInteractionSettings()
      .then((next) => {
        settings.value = normalizeAgentInteractionSettings(next);
        return settings.value;
      })
      .finally(() => {
        loadPromise = null;
      });
  }
  return loadPromise;
}

export async function loadCustomSubagentDefinitions(): Promise<CustomSubagentDefinition[]> {
  const next = await listCustomSubagents();
  subagents.value = [...next].sort((a, b) => a.name.localeCompare(b.name, "zh-CN"));
  return subagents.value;
}

export async function saveCustomSubagentDefinition(
  input: CustomSubagentUpsertInput,
): Promise<CustomSubagentDefinition> {
  const saved = await upsertCustomSubagent(input);
  const next = [...subagents.value];
  const index = next.findIndex((item) => item.id === saved.id);
  if (index === -1) next.push(saved);
  else next[index] = saved;
  subagents.value = next.sort((a, b) => a.name.localeCompare(b.name, "zh-CN"));
  return saved;
}

export async function removeCustomSubagentDefinition(id: string): Promise<void> {
  await deleteCustomSubagent(id);
  subagents.value = subagents.value.filter((item) => item.id !== id);
}

export async function updateAgentInteractionSettings(
  patch: Partial<AgentInteractionSettings>,
): Promise<AgentInteractionSettings> {
  const previous = settings.value;
  const next = normalizeAgentInteractionSettings({ ...previous, ...patch });
  if (
    next.nonInterruptMode === previous.nonInterruptMode &&
    next.debug === previous.debug &&
    next.permissionMode === previous.permissionMode &&
    sameCodexProfile(next.codexProfile, previous.codexProfile) &&
    sameSubagentMode(next.subagentMode, previous.subagentMode) &&
    sameJsonValue(next.autoTurnDecision, previous.autoTurnDecision)
  ) {
    return previous;
  }
  settings.value = next;
  try {
    await setAgentInteractionSettings(next);
    return next;
  } catch (err) {
    settings.value = previous;
    throw err;
  }
}

export function useAgentInteractionSettings() {
  return {
    settings: readonly(settings),
    subagents: readonly(subagents),
    nonInterruptMode: computed(() => settings.value.nonInterruptMode),
    debug: computed(() => settings.value.debug),
    permissionMode: computed(() => settings.value.permissionMode),
    load: loadAgentInteractionSettings,
    loadSubagents: loadCustomSubagentDefinitions,
    saveSubagent: saveCustomSubagentDefinition,
    deleteSubagent: removeCustomSubagentDefinition,
    update: updateAgentInteractionSettings,
  };
}
