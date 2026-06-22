export function createProviderHelpers(providerCodexJson, chatBackendsJson) {
  const providerCodex = deepFreeze(providerCodexJson);
  const chatBackends = deepFreeze(chatBackendsJson);
  const reasoningEfforts = chatBackends.reasoningEfforts;
  const backendReasoningEfforts = chatBackends.backendReasoningEfforts;
  const codexReasoningEfforts = chatBackends.backendReasoningEfforts.codex;
  const codexSettingsProfiles = providerCodex.codexSettingsProfiles;
  const defaultCodexProfileSettings = deepFreeze({
    profile: providerCodex.defaultCodexSettingsProfile,
    model: null,
    reasoningEffort: null,
    runtimeWorkspaceRoots: [],
    responsesApiClientMetadata: null,
    additionalContext: null,
    persistExtendedHistory: null,
    initialTurnsPage: null,
    excludeTurns: [],
  });
  const reasoningEffortSet = new Set(reasoningEfforts);
  const codexReasoningEffortSet = new Set(codexReasoningEfforts);
  const codexSettingsProfileSet = new Set(codexSettingsProfiles);

  function isReasoningEffort(value) {
    return typeof value === "string" && reasoningEffortSet.has(value);
  }

  function normalizeReasoningEffort(value) {
    return isReasoningEffort(value) ? value : null;
  }

  function reasoningEffortsForBackend(backend) {
    return backendReasoningEfforts[backend] || [];
  }

  function normalizeReasoningEffortForBackend(backend, value) {
    const effort = normalizeReasoningEffort(value);
    if (!effort) return null;
    const supportedEfforts = reasoningEffortsForBackend(backend);
    if (supportedEfforts.includes(effort)) return effort;
    const effortIndex = reasoningEfforts.indexOf(effort);
    for (let index = effortIndex - 1; index >= 0; index -= 1) {
      const candidate = reasoningEfforts[index];
      if (supportedEfforts.includes(candidate)) return candidate;
    }
    return supportedEfforts[0] || null;
  }

  function isCodexReasoningEffort(value) {
    return typeof value === "string" && codexReasoningEffortSet.has(value);
  }

  function normalizeCodexReasoningEffort(value) {
    return isCodexReasoningEffort(value) ? value : null;
  }

  function isCodexSettingsProfile(value) {
    return typeof value === "string" && codexSettingsProfileSet.has(value);
  }

  function normalizeCodexSettingsProfile(value) {
    return isCodexSettingsProfile(value) ? value : defaultCodexProfileSettings.profile;
  }

  function normalizeCodexProfileSettings(input, base = defaultCodexProfileSettings) {
    return {
      profile: normalizeCodexSettingsProfile(input?.profile ?? base.profile),
      model: normalizeNullableText(input?.model ?? base.model),
      reasoningEffort: normalizeCodexReasoningEffort(input?.reasoningEffort ?? base.reasoningEffort),
      runtimeWorkspaceRoots: normalizeUniqueTrimmedStrings(input?.runtimeWorkspaceRoots ?? base.runtimeWorkspaceRoots),
      responsesApiClientMetadata: normalizeCodexJsonObject(
        input?.responsesApiClientMetadata ?? base.responsesApiClientMetadata,
      ),
      additionalContext: normalizeNullableText(input?.additionalContext ?? base.additionalContext),
      persistExtendedHistory: normalizeNullableBoolean(input?.persistExtendedHistory ?? base.persistExtendedHistory),
      initialTurnsPage: normalizeCodexJsonObject(input?.initialTurnsPage ?? base.initialTurnsPage),
      excludeTurns: normalizeUniqueTrimmedStrings(input?.excludeTurns ?? base.excludeTurns),
    };
  }

  return {
    REASONING_EFFORTS: reasoningEfforts,
    BACKEND_REASONING_EFFORTS: backendReasoningEfforts,
    CODEX_REASONING_EFFORTS: codexReasoningEfforts,
    CODEX_SETTINGS_PROFILES: codexSettingsProfiles,
    DEFAULT_CODEX_PROFILE_SETTINGS: defaultCodexProfileSettings,
    isReasoningEffort,
    normalizeReasoningEffort,
    reasoningEffortsForBackend,
    normalizeReasoningEffortForBackend,
    isCodexReasoningEffort,
    normalizeCodexReasoningEffort,
    isCodexSettingsProfile,
    normalizeCodexSettingsProfile,
    normalizeUniqueTrimmedStrings,
    normalizeCodexJsonObject,
    normalizeCodexProfileSettings,
  };
}

export function normalizeUniqueTrimmedStrings(value) {
  if (!Array.isArray(value)) return [];
  const items = value
    .filter((item) => typeof item === "string")
    .map((item) => item.trim())
    .filter(Boolean);
  return [...new Set(items)];
}

export function normalizeCodexJsonObject(value) {
  return value && typeof value === "object" && !Array.isArray(value)
    ? { ...value }
    : null;
}

function normalizeNullableText(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeNullableBoolean(value) {
  return typeof value === "boolean" ? value : null;
}

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  for (const child of Object.values(value)) {
    deepFreeze(child);
  }
  return Object.freeze(value);
}
