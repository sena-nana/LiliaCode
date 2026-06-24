import agentInteractionDefaults from "./agent-interaction-defaults.json" with { type: "json" };
import {
  DEFAULT_CODEX_PROFILE_SETTINGS,
  normalizeCodexProfileSettings,
} from "./providerRuntime.mjs";
import {
  isRuntimePermissionMode,
  normalizeRuntimePermissionMode,
} from "./permissionModes.mjs";

export const AUTO_TURN_DECISION_PERMISSION_OPTIONS = Object.freeze([
  { key: "allowModelTier", label: "模型层级", ariaLabel: "辅助模型操作模型层级" },
  { key: "allowReasoningEffort", label: "思考强度", ariaLabel: "辅助模型操作思考强度" },
  { key: "allowPlanMode", label: "计划模式", ariaLabel: "辅助模型操作计划模式" },
  { key: "allowGoalMode", label: "Goal 模式", ariaLabel: "辅助模型操作 Goal 模式" },
  { key: "allowSessionFork", label: "会话分叉", ariaLabel: "辅助模型操作会话分叉" },
]);

export const AUTO_TURN_DECISION_PERMISSION_KEYS = Object.freeze(
  AUTO_TURN_DECISION_PERMISSION_OPTIONS.map((option) => option.key),
);

const MAIN_AGENT_PROMPT_MODES = Object.freeze(["conservative", "aggressive", "custom"]);
const MAIN_AGENT_PROMPT_MODE_SET = new Set(MAIN_AGENT_PROMPT_MODES);

const defaults = readAgentInteractionDefaultsManifest(agentInteractionDefaults);

export const DEFAULT_AGENT_SUBAGENT_MODE_SETTINGS = freezeSubagentModeSettings(
  defaults.subagentMode,
);
export const DEFAULT_AUTO_TURN_DECISION_SETTINGS = Object.freeze({
  ...defaults.autoTurnDecision,
});
export const DEFAULT_AGENT_INTERACTION_SETTINGS = freezeAgentInteractionSettings(defaults);

export function normalizeMainAgentPromptMode(value, fallback = "conservative") {
  return typeof value === "string" && MAIN_AGENT_PROMPT_MODE_SET.has(value)
    ? value
    : fallback;
}

export function normalizeMainAgentCustomPrompt(value, fallback = "") {
  return typeof value === "string" ? value.trim() : fallback;
}

export function normalizeAgentSubagentModeSettings(
  input,
  base = DEFAULT_AGENT_SUBAGENT_MODE_SETTINGS,
) {
  return {
    enabled: typeof input?.enabled === "boolean" ? input.enabled : base.enabled,
    codex: {
      enabled: typeof input?.codex?.enabled === "boolean"
        ? input.codex.enabled
        : base.codex.enabled,
    },
    claude: {
      enabled: typeof input?.claude?.enabled === "boolean"
        ? input.claude.enabled
        : base.claude.enabled,
      forwardSubagentText: typeof input?.claude?.forwardSubagentText === "boolean"
        ? input.claude.forwardSubagentText
        : base.claude.forwardSubagentText,
      agentProgressSummaries: typeof input?.claude?.agentProgressSummaries === "boolean"
        ? input.claude.agentProgressSummaries
        : base.claude.agentProgressSummaries,
    },
  };
}

export function normalizeAutoTurnDecisionSettings(
  input,
  base = DEFAULT_AUTO_TURN_DECISION_SETTINGS,
) {
  const normalized = {
    enabled: typeof input?.enabled === "boolean" ? input.enabled : base.enabled,
  };
  for (const key of AUTO_TURN_DECISION_PERMISSION_KEYS) {
    normalized[key] = typeof input?.[key] === "boolean" ? input[key] : base[key];
  }
  return normalized;
}

export function normalizeAgentInteractionSettings(
  input,
  base = DEFAULT_AGENT_INTERACTION_SETTINGS,
) {
  return {
    nonInterruptMode: typeof input?.nonInterruptMode === "boolean"
      ? input.nonInterruptMode
      : base.nonInterruptMode,
    debug: typeof input?.debug === "boolean" ? input.debug : base.debug,
    permissionMode: normalizeRuntimePermissionMode(input?.permissionMode, base.permissionMode),
    mainAgentPromptMode: normalizeMainAgentPromptMode(
      input?.mainAgentPromptMode,
      base.mainAgentPromptMode,
    ),
    mainAgentCustomPrompt: normalizeMainAgentCustomPrompt(
      input?.mainAgentCustomPrompt,
      base.mainAgentCustomPrompt,
    ),
    codexProfile: normalizeCodexProfileSettings(input?.codexProfile, base.codexProfile),
    subagentMode: normalizeAgentSubagentModeSettings(input?.subagentMode, base.subagentMode),
    autoTurnDecision: normalizeAutoTurnDecisionSettings(
      input?.autoTurnDecision,
      base.autoTurnDecision,
    ),
  };
}

function readAgentInteractionDefaultsManifest(value) {
  const row = requireManifestRecord(value, "agent-interaction-defaults.json");
  if (!isRuntimePermissionMode(row.permissionMode)) {
    throw new Error("agent-interaction-defaults.json.permissionMode must be a permission mode");
  }
  const codexProfile = requireManifestRecord(
    row.codexProfile,
    "agent-interaction-defaults.json.codexProfile",
  );
  const subagentMode = requireManifestRecord(
    row.subagentMode,
    "agent-interaction-defaults.json.subagentMode",
  );
  const autoTurnDecision = requireManifestRecord(
    row.autoTurnDecision,
    "agent-interaction-defaults.json.autoTurnDecision",
  );
  return {
    nonInterruptMode: booleanManifestField(
      row,
      "nonInterruptMode",
      "agent-interaction-defaults.json",
    ),
    debug: booleanManifestField(row, "debug", "agent-interaction-defaults.json"),
    permissionMode: row.permissionMode,
    mainAgentPromptMode: normalizeMainAgentPromptMode(row.mainAgentPromptMode),
    mainAgentCustomPrompt: normalizeMainAgentCustomPrompt(row.mainAgentCustomPrompt),
    codexProfile: normalizeCodexProfileSettings(
      codexProfile,
      DEFAULT_CODEX_PROFILE_SETTINGS,
    ),
    subagentMode: readAgentSubagentModeDefaults(subagentMode),
    autoTurnDecision: readAutoTurnDecisionDefaults(autoTurnDecision),
  };
}

function readAgentSubagentModeDefaults(row) {
  const codex = requireManifestRecord(
    row.codex,
    "agent-interaction-defaults.json.subagentMode.codex",
  );
  const claude = requireManifestRecord(
    row.claude,
    "agent-interaction-defaults.json.subagentMode.claude",
  );
  return {
    enabled: booleanManifestField(
      row,
      "enabled",
      "agent-interaction-defaults.json.subagentMode",
    ),
    codex: {
      enabled: booleanManifestField(
        codex,
        "enabled",
        "agent-interaction-defaults.json.subagentMode.codex",
      ),
    },
    claude: {
      enabled: booleanManifestField(
        claude,
        "enabled",
        "agent-interaction-defaults.json.subagentMode.claude",
      ),
      forwardSubagentText: booleanManifestField(
        claude,
        "forwardSubagentText",
        "agent-interaction-defaults.json.subagentMode.claude",
      ),
      agentProgressSummaries: booleanManifestField(
        claude,
        "agentProgressSummaries",
        "agent-interaction-defaults.json.subagentMode.claude",
      ),
    },
  };
}

function readAutoTurnDecisionDefaults(row) {
  const path = "agent-interaction-defaults.json.autoTurnDecision";
  const defaults = {
    enabled: booleanManifestField(row, "enabled", path),
  };
  for (const key of AUTO_TURN_DECISION_PERMISSION_KEYS) {
    defaults[key] = booleanManifestField(row, key, path);
  }
  return defaults;
}

function requireManifestRecord(value, path) {
  const row = recordValue(value);
  if (!row) throw new Error(`${path} must be an object`);
  return row;
}

function booleanManifestField(row, key, path) {
  const value = row[key];
  if (typeof value !== "boolean") throw new Error(`${path}.${key} must be a boolean`);
  return value;
}

function freezeSubagentModeSettings(value) {
  return Object.freeze({
    enabled: value.enabled,
    codex: Object.freeze({ ...value.codex }),
    claude: Object.freeze({ ...value.claude }),
  });
}

function freezeAgentInteractionSettings(value) {
  return Object.freeze({
    nonInterruptMode: value.nonInterruptMode,
    debug: value.debug,
    permissionMode: value.permissionMode,
    mainAgentPromptMode: value.mainAgentPromptMode,
    mainAgentCustomPrompt: value.mainAgentCustomPrompt,
    codexProfile: Object.freeze({ ...value.codexProfile }),
    subagentMode: freezeSubagentModeSettings(value.subagentMode),
    autoTurnDecision: Object.freeze({ ...value.autoTurnDecision }),
  });
}

function recordValue(value) {
  return value && typeof value === "object" && !Array.isArray(value) ? value : null;
}
