import { computed, readonly, ref } from "vue";
import type { AgentInteractionSettings } from "@lilia/contracts";
import {
  getAgentInteractionSettings,
  setAgentInteractionSettings,
} from "../services/chat";

const DEFAULT_AGENT_INTERACTION_SETTINGS: AgentInteractionSettings = {
  nonInterruptMode: false,
  debug: false,
  agentRuntimeChannel: "builtin",
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
};

const settings = ref<AgentInteractionSettings>({
  ...DEFAULT_AGENT_INTERACTION_SETTINGS,
});

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

export function uniqueTrimmedLines(value: string): string[] {
  return uniqueTrimmedStrings(value.split(/\r?\n/));
}

export function sameOrderedStrings(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((item, index) => item === b[index]);
}

export function normalizeAgentInteractionSettings(
  input: Partial<AgentInteractionSettings> | null | undefined,
): AgentInteractionSettings {
  const codexProfile = input?.codexProfile;
  return {
    nonInterruptMode: input?.nonInterruptMode === true,
    debug: input?.debug === true,
    agentRuntimeChannel: normalizeRuntimeChannel(input?.agentRuntimeChannel),
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
  };
}

function normalizeRuntimeChannel(value: unknown): AgentInteractionSettings["agentRuntimeChannel"] {
  return value === "mutsuki_core" ? "mutsuki_core" : "builtin";
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

export function sameCodexProfile(
  a: unknown,
  b: unknown,
): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
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

export async function updateAgentInteractionSettings(
  patch: Partial<AgentInteractionSettings>,
): Promise<AgentInteractionSettings> {
  const previous = settings.value;
  const next = normalizeAgentInteractionSettings({ ...previous, ...patch });
  if (
    next.nonInterruptMode === previous.nonInterruptMode &&
    next.debug === previous.debug &&
    next.agentRuntimeChannel === previous.agentRuntimeChannel &&
    sameCodexProfile(next.codexProfile, previous.codexProfile)
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
    nonInterruptMode: computed(() => settings.value.nonInterruptMode),
    debug: computed(() => settings.value.debug),
    agentRuntimeChannel: computed(() => settings.value.agentRuntimeChannel),
    load: loadAgentInteractionSettings,
    update: updateAgentInteractionSettings,
  };
}
