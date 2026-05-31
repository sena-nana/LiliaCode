import { computed, readonly, ref } from "vue";
import type { AgentInteractionSettings } from "@lilia/contracts";
import {
  getAgentInteractionSettings,
  setAgentInteractionSettings,
} from "../services/chat";

const DEFAULT_AGENT_INTERACTION_SETTINGS: AgentInteractionSettings = {
  nonInterruptMode: false,
  debug: false,
};

const settings = ref<AgentInteractionSettings>({
  ...DEFAULT_AGENT_INTERACTION_SETTINGS,
});

let loadPromise: Promise<AgentInteractionSettings> | null = null;

function normalizeAgentInteractionSettings(
  input: Partial<AgentInteractionSettings> | null | undefined,
): AgentInteractionSettings {
  return {
    nonInterruptMode: input?.nonInterruptMode === true,
    debug: input?.debug === true,
  };
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
    next.debug === previous.debug
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
