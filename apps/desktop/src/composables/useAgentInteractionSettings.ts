import { computed, readonly, ref } from "vue";
import type { AgentInteractionSettings } from "@lilia/contracts";
import {
  getAgentInteractionSettings,
  setAgentInteractionSettings,
} from "../services/chat";

const DEFAULT_AGENT_INTERACTION_SETTINGS: AgentInteractionSettings = {
  nonInterruptMode: false,
  debug: false,
  codexProfile: {
    profile: "default",
    model: null,
    reasoningEffort: null,
    runtimeWorkspaceRoots: [],
    permissions: { profile: "default" },
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
    },
  };
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
    load: loadAgentInteractionSettings,
    update: updateAgentInteractionSettings,
  };
}
