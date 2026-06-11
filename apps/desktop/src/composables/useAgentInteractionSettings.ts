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
    permissions: { profile: "default" },
    responsesApiClientMetadata: null,
    additionalContext: null,
    persistExtendedHistory: null,
    initialTurnsPage: null,
    excludeTurns: [],
    commandExecPermissionProfile: null,
  },
};

const settings = ref<AgentInteractionSettings>({
  ...DEFAULT_AGENT_INTERACTION_SETTINGS,
});

let loadPromise: Promise<AgentInteractionSettings> | null = null;

function normalizeAgentInteractionSettings(
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
      runtimeWorkspaceRoots: Array.isArray(codexProfile?.runtimeWorkspaceRoots)
        ? Array.from(new Set(codexProfile.runtimeWorkspaceRoots.map((root) => root.trim()).filter(Boolean)))
        : [],
      permissions: {
        profile: normalizePermissionProfile(codexProfile?.permissions?.profile),
      },
      responsesApiClientMetadata: normalizeJsonObject(codexProfile?.responsesApiClientMetadata),
      additionalContext: normalizeNullableText(codexProfile?.additionalContext),
      persistExtendedHistory: normalizeNullableBoolean(codexProfile?.persistExtendedHistory),
      initialTurnsPage: normalizeJsonObject(codexProfile?.initialTurnsPage),
      excludeTurns: Array.isArray(codexProfile?.excludeTurns)
        ? Array.from(new Set(codexProfile.excludeTurns.map((turn) => turn.trim()).filter(Boolean)))
        : [],
      commandExecPermissionProfile: normalizeOptionalPermissionProfile(
        codexProfile?.commandExecPermissionProfile,
      ),
    },
  };
}

function normalizeRuntimeChannel(value: unknown): AgentInteractionSettings["agentRuntimeChannel"] {
  return value === "nanobot" ? "nanobot" : "builtin";
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

function normalizePermissionProfile(
  value: unknown,
): AgentInteractionSettings["codexProfile"]["permissions"]["profile"] {
  return value === "readOnly" || value === "workspaceWrite" || value === "dangerFullAccess"
    ? value
    : "default";
}

function normalizeOptionalPermissionProfile(
  value: unknown,
): AgentInteractionSettings["codexProfile"]["commandExecPermissionProfile"] {
  return value === "default" || value === "readOnly" || value === "workspaceWrite" || value === "dangerFullAccess"
    ? value
    : null;
}

function sameCodexProfile(
  a: AgentInteractionSettings["codexProfile"],
  b: AgentInteractionSettings["codexProfile"],
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
