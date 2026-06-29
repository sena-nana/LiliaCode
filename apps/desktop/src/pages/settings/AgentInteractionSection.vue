<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, type Component, watch } from "vue";
import {
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  Sparkles,
} from "@lucide/vue";
import type {
  AgentInteractionSettings,
  MainAgentPromptMode,
  PermissionMode,
} from "@lilia/contracts";
import {
  AUTO_TURN_DECISION_PERMISSION_OPTIONS,
  LOCKED_PERMISSION_MODES,
  PERMISSION_MODE_DISPLAY,
  PERMISSION_MODE_DISPLAY_ORDER,
  PROMPT_MAIN_AGENT,
} from "@lilia/contracts";
import {
  useAgentInteractionSettings,
} from "../../composables/useAgentInteractionSettings";
import {
  beginPerfStage,
  cancelIdleRun,
  measurePerfAsync,
  runWhenIdle,
  scheduleAfterPaint,
} from "../../utils/perf";
import { createLazyLoadState } from "../../utils/lazyLoadState";

const subagentCatalogSectionLoad = createLazyLoadState<Component>(() =>
  measurePerfAsync(
    "settings.agent.subagents.load",
    async () => (await import("./SubagentCatalogSection.vue")).default as Component,
  )
);

const SubagentCatalogSection = defineAsyncComponent({
  suspensible: false,
  loader: () => subagentCatalogSectionLoad.load(),
});

const agentInteractionStore = useAgentInteractionSettings();
const agentInteraction = agentInteractionStore.settings;
const savingAgentInteraction = ref(false);
const agentInteractionError = ref<string | null>(null);
const mainAgentCustomPromptDraft = ref("");
const subagentCardExpanded = ref(false);
const subagentCatalogReady = ref(false);
const subagentDetailsId = "agent-subagent-mode-details";
let subagentCatalogIdleHandle: number | null = null;
let cancelSubagentCatalogPaint: (() => void) | null = null;
let subagentCatalogMountSeq = 0;
let disposed = false;

const lockedPermissionModes = new Set<PermissionMode>(LOCKED_PERMISSION_MODES);
const askReadonlyPermissionModes: readonly [PermissionMode, PermissionMode] = ["ask", "readonly"];
type PermissionAvailabilityOption = {
  id: string;
  mode: PermissionMode | null;
  currentModes: readonly PermissionMode[];
  label: string;
  description: string;
  locked: boolean;
};
const permissionAvailabilityOptions: PermissionAvailabilityOption[] = [
  {
    id: "ask-readonly",
    mode: null,
    currentModes: askReadonlyPermissionModes,
    label: "默认权限",
    description: "允许读取与分析，涉及变更时等待确认。",
    locked: askReadonlyPermissionModes.every((mode) => lockedPermissionModes.has(mode)),
  },
  ...PERMISSION_MODE_DISPLAY_ORDER
    .filter((value) => value !== "ask" && value !== "readonly")
    .map((value) => ({
      id: value,
      mode: value,
      currentModes: [value],
      label: PERMISSION_MODE_DISPLAY[value].label,
      description: PERMISSION_MODE_DISPLAY[value].description,
      locked: lockedPermissionModes.has(value),
    })),
];

type SubagentModePatch = Partial<Omit<AgentInteractionSettings["subagentMode"], "codex" | "claude">> & {
  codex?: Partial<AgentInteractionSettings["subagentMode"]["codex"]>;
  claude?: Partial<AgentInteractionSettings["subagentMode"]["claude"]>;
};

const autoTurnDecisionPermissionOptions = AUTO_TURN_DECISION_PERMISSION_OPTIONS;
const mainAgentWorkflowsPrompt = PROMPT_MAIN_AGENT.workflowOrder
  .map((key) => PROMPT_MAIN_AGENT.workflowTypes[key])
  .filter((workflow): workflow is NonNullable<typeof workflow> => Boolean(workflow))
  .map((workflow) => `## ${workflow.title.trim()}\n${workflow.summary.trim()}\n\n${workflow.prompt.trim()}`)
  .filter((part) => part.trim())
  .join("\n\n");
const mainAgentPromptModeOptions: Array<{
  value: MainAgentPromptMode;
  label: string;
  description: string;
}> = [
  {
    value: "conservative",
    label: "保守",
    description: "小步低风险，优先沿用现有结构。",
  },
  {
    value: "aggressive",
    label: "激进",
    description: "允许根因级检查和必要功能重构。",
  },
  {
    value: "custom",
    label: "自定义",
    description: "使用自己填写的策略片段，保留内置工具与工作流提示。",
  },
];

function mainAgentStrategyPrompt(mode: MainAgentPromptMode, customPrompt: string): string {
  if (mode === "custom") {
    const trimmed = customPrompt.trim();
    if (trimmed) return trimmed;
    return PROMPT_MAIN_AGENT.modes.conservative.trim();
  }
  return PROMPT_MAIN_AGENT.modes[mode].trim();
}

function buildMainAgentPromptPreview(mode: MainAgentPromptMode, customPrompt: string): string {
  return [
    PROMPT_MAIN_AGENT.basePrompt.trim(),
    mainAgentStrategyPrompt(mode, customPrompt),
    PROMPT_MAIN_AGENT.toolsPrompt.trim(),
    mainAgentWorkflowsPrompt,
  ]
    .filter((part) => part.trim())
    .join("\n\n");
}

function activeMainAgentPromptDescription(): string {
  return mainAgentPromptModeOptions.find((option) => option.value === agentInteraction.value.mainAgentPromptMode)
    ?.description ?? mainAgentPromptModeOptions[0].description;
}

function defaultCustomMainAgentPrompt(): string {
  const persisted = agentInteraction.value.mainAgentCustomPrompt.trim();
  if (persisted) return persisted;
  const currentMode = agentInteraction.value.mainAgentPromptMode;
  if (currentMode === "custom") return PROMPT_MAIN_AGENT.modes.conservative.trim();
  return mainAgentStrategyPrompt(currentMode, "");
}

const mainAgentPromptPreview = computed(() =>
  buildMainAgentPromptPreview(
    agentInteraction.value.mainAgentPromptMode,
    agentInteraction.value.mainAgentPromptMode === "custom"
      ? mainAgentCustomPromptDraft.value
      : agentInteraction.value.mainAgentCustomPrompt,
  )
);

async function loadAgentInteraction() {
  try {
    await agentInteractionStore.load();
  } catch (err) {
    if (disposed) return;
    agentInteractionError.value = `读取 Agent 交互设置失败：${String(err)}`;
  }
}

async function setNonInterruptMode(nonInterruptMode: boolean) {
  await setAgentInteraction({ nonInterruptMode });
}

async function setDebugMode(debug: boolean) {
  await setAgentInteraction({ debug });
}

async function setPermissionModeAvailable(permissionMode: PermissionMode, enabled: boolean) {
  if (lockedPermissionModes.has(permissionMode)) return;
  await setAgentInteraction({
    permissionModeAvailability: {
      ...agentInteraction.value.permissionModeAvailability,
      [permissionMode]: enabled,
    },
  });
}

function setPermissionAvailabilityOptionAvailable(option: PermissionAvailabilityOption, enabled: boolean) {
  if (option.mode === null) return;
  return setPermissionModeAvailable(option.mode, enabled);
}

function isPermissionAvailabilityOptionCurrent(option: PermissionAvailabilityOption) {
  return option.currentModes.includes(agentInteraction.value.permissionMode);
}

function isPermissionAvailabilityOptionEnabled(option: PermissionAvailabilityOption) {
  return option.mode !== null && agentInteraction.value.permissionModeAvailability[option.mode];
}

async function setMainAgentPromptMode(mainAgentPromptMode: MainAgentPromptMode) {
  if (mainAgentPromptMode === "custom") {
    const mainAgentCustomPrompt = defaultCustomMainAgentPrompt();
    mainAgentCustomPromptDraft.value = mainAgentCustomPrompt;
    await setAgentInteraction({ mainAgentPromptMode, mainAgentCustomPrompt });
    return;
  }
  await setAgentInteraction({ mainAgentPromptMode });
}

async function saveMainAgentCustomPrompt() {
  await setAgentInteraction({
    mainAgentPromptMode: "custom",
    mainAgentCustomPrompt: mainAgentCustomPromptDraft.value,
  });
}

function nextSubagentMode(
  patch: SubagentModePatch,
): AgentInteractionSettings["subagentMode"] {
  const current = agentInteraction.value.subagentMode;
  return {
    ...current,
    ...patch,
    codex: {
      ...current.codex,
      ...patch.codex,
    },
    claude: {
      ...current.claude,
      ...patch.claude,
    },
  };
}

async function setSubagentModeEnabled(enabled: boolean) {
  await setAgentInteraction({
    subagentMode: nextSubagentMode({ enabled }),
  });
}

async function setCodexSubagentEnabled(enabled: boolean) {
  await setAgentInteraction({
    subagentMode: nextSubagentMode({
      codex: { enabled },
    }),
  });
}

async function setClaudeSubagentField(
  key: "enabled" | "forwardSubagentText" | "agentProgressSummaries",
  value: boolean,
) {
  await setAgentInteraction({
    subagentMode: nextSubagentMode({
      claude: { ...agentInteraction.value.subagentMode.claude, [key]: value },
    }),
  });
}

function nextAutoTurnDecision(
  patch: Partial<AgentInteractionSettings["autoTurnDecision"]>,
): AgentInteractionSettings["autoTurnDecision"] {
  return {
    ...agentInteraction.value.autoTurnDecision,
    ...patch,
  };
}

async function setAutoTurnDecisionField(
  key: keyof AgentInteractionSettings["autoTurnDecision"],
  value: boolean,
) {
  await setAgentInteraction({
    autoTurnDecision: nextAutoTurnDecision({ [key]: value }),
  });
}

async function setAgentInteraction(patch: Partial<AgentInteractionSettings>) {
  if (disposed) return;
  savingAgentInteraction.value = true;
  agentInteractionError.value = null;
  try {
    await agentInteractionStore.update(patch);
  } catch (err) {
    if (!disposed) agentInteractionError.value = `保存 Agent 交互设置失败：${String(err)}`;
  } finally {
    if (!disposed) savingAgentInteraction.value = false;
  }
}

function toggleSubagentCard() {
  if (!agentInteraction.value.subagentMode.enabled) return;
  subagentCardExpanded.value = !subagentCardExpanded.value;
}

function scheduleSubagentCatalogMount() {
  if (subagentCatalogReady.value) return;
  cancelSubagentCatalogPaint?.();
  cancelSubagentCatalogPaint = null;
  const seq = ++subagentCatalogMountSeq;
  const stage = beginPerfStage("settings.agent.subagents.mount");
  cancelSubagentCatalogPaint = scheduleAfterPaint(() => {
    cancelSubagentCatalogPaint = null;
    if (seq !== subagentCatalogMountSeq || subagentCatalogReady.value) {
      stage.end("cancelled");
      return;
    }
    subagentCatalogIdleHandle = runWhenIdle(() => {
      subagentCatalogIdleHandle = null;
      if (seq !== subagentCatalogMountSeq || subagentCatalogReady.value) {
        stage.end("cancelled");
        return;
      }
      subagentCatalogReady.value = true;
      stage.end("idle");
    });
  });
}

watch(
  () => agentInteraction.value.subagentMode.enabled,
  (enabled) => {
    subagentCardExpanded.value = enabled;
  },
  { immediate: true },
);

watch(
  () => agentInteraction.value.mainAgentCustomPrompt,
  (prompt) => {
    mainAgentCustomPromptDraft.value = prompt;
  },
  { immediate: true },
);

onMounted(() => {
  disposed = false;
  void loadAgentInteraction();
  scheduleSubagentCatalogMount();
});

onBeforeUnmount(() => {
  disposed = true;
  subagentCatalogMountSeq += 1;
  cancelSubagentCatalogPaint?.();
  cancelSubagentCatalogPaint = null;
  if (subagentCatalogIdleHandle === null) return;
  cancelIdleRun(subagentCatalogIdleHandle);
  subagentCatalogIdleHandle = null;
});
</script>

<template>
  <div class="agent-settings-stack" data-agent-id="settings.agent">
    <section class="card agent-settings-card" aria-label="权限行为">
      <h2>
        <span class="card-h2__title">
          <Sparkles :size="14" aria-hidden="true" />
          权限行为
        </span>
      </h2>
      <div class="permission-toggle-list">
        <div
          v-for="option in permissionAvailabilityOptions"
          :key="option.id"
          class="permission-toggle-item"
          :data-agent-id="`settings.agent.permission-availability.${option.id}`"
        >
          <div class="permission-toggle-item__content">
            <div class="settings-row__label">
              {{ option.label }}
              <span
                v-if="isPermissionAvailabilityOptionCurrent(option)"
                class="permission-toggle-item__badge"
              >
                当前默认
              </span>
            </div>
            <p class="permission-toggle-item__hint">{{ option.description }}</p>
          </div>
          <button
            v-if="option.locked"
            type="button"
            class="ui-button ui-button--ghost permission-toggle-item__locked"
            :data-agent-id="`settings.agent.permission-availability.${option.id}.locked`"
            disabled
          >
            固定启用
          </button>
          <div v-else class="ui-segmented" role="radiogroup" :aria-label="`${option.label}权限可用性`">
            <button
              type="button"
              role="radio"
              :aria-checked="!isPermissionAvailabilityOptionEnabled(option)"
              :data-agent-id="`settings.agent.permission-availability.${option.id}.off`"
              :class="{ 'is-active': !isPermissionAvailabilityOptionEnabled(option) }"
              :disabled="savingAgentInteraction"
              @click="setPermissionAvailabilityOptionAvailable(option, false)"
            >
              关闭
            </button>
            <button
              type="button"
              role="radio"
              :aria-checked="isPermissionAvailabilityOptionEnabled(option)"
              :data-agent-id="`settings.agent.permission-availability.${option.id}.on`"
              :class="{ 'is-active': isPermissionAvailabilityOptionEnabled(option) }"
              :disabled="savingAgentInteraction"
              @click="setPermissionAvailabilityOptionAvailable(option, true)"
            >
              启用
            </button>
          </div>
        </div>
      </div>
    </section>

    <section class="card agent-settings-card" aria-label="主 Agent 策略">
      <h2>主 Agent 策略</h2>
      <div class="settings-row">
      <div class="settings-row__label">策略模式</div>
      <div class="settings-row__control settings-row__control--loose">
        <div class="ui-segmented" role="radiogroup" aria-label="主 Agent 策略">
          <button
            v-for="option in mainAgentPromptModeOptions"
            :key="option.value"
            type="button"
            role="radio"
            :aria-checked="agentInteraction.mainAgentPromptMode === option.value"
            :data-agent-id="`settings.agent.main-prompt-mode.${option.value}`"
            :class="{ 'is-active': agentInteraction.mainAgentPromptMode === option.value }"
            :disabled="savingAgentInteraction"
            @click="setMainAgentPromptMode(option.value)"
          >
            {{ option.label }}
          </button>
        </div>
        <span class="settings-row__status muted">
          {{ activeMainAgentPromptDescription() }}
        </span>
      </div>
      </div>

      <section class="main-agent-prompt-panel" aria-label="主 Agent 工作流提示">
      <div class="main-agent-prompt-panel__head">
        <div class="settings-row__label">主 Agent 工作流提示预览</div>
        <span class="main-agent-prompt-panel__meta">
          {{ agentInteraction.mainAgentPromptMode === "custom" ? "自定义策略" : "内置策略" }}
        </span>
      </div>
      <pre class="main-agent-prompt-panel__preview">{{ mainAgentPromptPreview }}</pre>
      <div
        v-if="agentInteraction.mainAgentPromptMode === 'custom'"
        class="main-agent-prompt-panel__editor"
      >
        <label class="settings-row__label" for="main-agent-custom-prompt">
          自定义主 Agent 提示词
        </label>
        <textarea
          id="main-agent-custom-prompt"
          v-model="mainAgentCustomPromptDraft"
          class="ui-textarea main-agent-prompt-panel__textarea"
          data-agent-id="settings.agent.main-prompt.custom"
          rows="8"
          :disabled="savingAgentInteraction"
        />
        <button
          type="button"
          class="ui-button ui-button--ghost main-agent-prompt-panel__save"
          data-agent-id="settings.agent.main-prompt.save"
          :disabled="savingAgentInteraction"
          @click="saveMainAgentCustomPrompt"
        >
          应用自定义提示词
        </button>
      </div>
      </section>
    </section>

    <section class="card agent-settings-card" aria-label="运行配置">
      <h2>运行配置</h2>
      <div class="settings-row">
      <div class="settings-row__label">非打断模式</div>
      <div class="ui-segmented" role="radiogroup" aria-label="非打断模式">
        <button
          type="button"
          role="radio"
          :aria-checked="!agentInteraction.nonInterruptMode"
          data-agent-id="settings.agent.non-interrupt.off"
          :class="{ 'is-active': !agentInteraction.nonInterruptMode }"
          :disabled="savingAgentInteraction"
          @click="setNonInterruptMode(false)"
        >
          关闭
        </button>
        <button
          type="button"
          role="radio"
          :aria-checked="agentInteraction.nonInterruptMode"
          data-agent-id="settings.agent.non-interrupt.on"
          :class="{ 'is-active': agentInteraction.nonInterruptMode }"
          :disabled="savingAgentInteraction"
          @click="setNonInterruptMode(true)"
        >
          开启
        </button>
      </div>
      </div>

      <div class="settings-row">
      <div class="settings-row__label">Debug 面板</div>
      <div class="ui-segmented" role="radiogroup" aria-label="Debug 面板">
        <button
          type="button"
          role="radio"
          :aria-checked="!agentInteraction.debug"
          data-agent-id="settings.agent.debug.off"
          :class="{ 'is-active': !agentInteraction.debug }"
          :disabled="savingAgentInteraction"
          @click="setDebugMode(false)"
        >
          关闭
        </button>
        <button
          type="button"
          role="radio"
          :aria-checked="agentInteraction.debug"
          data-agent-id="settings.agent.debug.on"
          :class="{ 'is-active': agentInteraction.debug }"
          :disabled="savingAgentInteraction"
          @click="setDebugMode(true)"
        >
          开启
        </button>
      </div>
      </div>
    </section>

    <section class="card auto-turn-card" aria-label="自动轮次策略">
      <div class="auto-turn-card__head">
        <div>
          <h2>自动轮次策略</h2>
          <p class="auto-turn-card__hint">Auto 模式下，由辅助模型在轮次启动前决定本轮执行策略。</p>
        </div>
        <div class="ui-segmented" role="radiogroup" aria-label="自动轮次策略">
          <button
          type="button"
          role="radio"
          :aria-checked="!agentInteraction.autoTurnDecision.enabled"
          data-agent-id="settings.agent.auto-turn.off"
          :class="{ 'is-active': !agentInteraction.autoTurnDecision.enabled }"
            :disabled="savingAgentInteraction"
            @click="setAutoTurnDecisionField('enabled', false)"
          >
            关闭
          </button>
          <button
          type="button"
          role="radio"
          :aria-checked="agentInteraction.autoTurnDecision.enabled"
          data-agent-id="settings.agent.auto-turn.on"
          :class="{ 'is-active': agentInteraction.autoTurnDecision.enabled }"
            :disabled="savingAgentInteraction"
            @click="setAutoTurnDecisionField('enabled', true)"
          >
            开启
          </button>
        </div>
      </div>

      <div class="auto-turn-card__grid">
        <div
          v-for="option in autoTurnDecisionPermissionOptions"
          :key="option.key"
          class="settings-row settings-row--nested"
        >
          <div class="settings-row__label">{{ option.label }}</div>
          <div class="ui-segmented" role="radiogroup" :aria-label="option.ariaLabel">
            <button
            type="button"
            role="radio"
            :aria-checked="!agentInteraction.autoTurnDecision[option.key]"
            :data-agent-id="`settings.agent.auto-turn.${option.key}.off`"
            :class="{ 'is-active': !agentInteraction.autoTurnDecision[option.key] }"
              :disabled="savingAgentInteraction || !agentInteraction.autoTurnDecision.enabled"
              @click="setAutoTurnDecisionField(option.key, false)"
            >
              禁止
            </button>
            <button
            type="button"
            role="radio"
            :aria-checked="agentInteraction.autoTurnDecision[option.key]"
            :data-agent-id="`settings.agent.auto-turn.${option.key}.on`"
            :class="{ 'is-active': agentInteraction.autoTurnDecision[option.key] }"
              :disabled="savingAgentInteraction || !agentInteraction.autoTurnDecision.enabled"
              @click="setAutoTurnDecisionField(option.key, true)"
            >
              允许
            </button>
          </div>
        </div>
      </div>
    </section>

    <section
      class="card subagent-mode-card"
    >
      <div
        class="subagent-mode-card__header"
        data-agent-id="settings.agent-interaction.subagent-details.toggle"
        role="button"
        :tabindex="agentInteraction.subagentMode.enabled ? 0 : -1"
        :aria-controls="subagentDetailsId"
        :aria-expanded="subagentCardExpanded"
        :aria-disabled="!agentInteraction.subagentMode.enabled"
        :aria-label="subagentCardExpanded ? '收起 Subagent 详细配置' : '展开 Subagent 详细配置'"
        @click="toggleSubagentCard"
        @keydown.enter.prevent="toggleSubagentCard"
        @keydown.space.prevent="toggleSubagentCard"
      >
        <div class="subagent-mode-card__summary">
          <div class="settings-row__label">Subagent 模式</div>
          <p class="subagent-mode-card__hint">启用后可展开配置 Codex 与 Claude 的子代理行为。</p>
        </div>

        <div
          class="ui-segmented subagent-mode-card__switch"
          role="radiogroup"
          aria-label="Subagent 模式"
          @click.stop
        >
          <button
          type="button"
          role="radio"
          :aria-checked="!agentInteraction.subagentMode.enabled"
          data-agent-id="settings.agent.subagent.off"
          :class="{ 'is-active': !agentInteraction.subagentMode.enabled }"
            :disabled="savingAgentInteraction"
            @click="setSubagentModeEnabled(false)"
          >
            关闭
          </button>
          <button
          type="button"
          role="radio"
          :aria-checked="agentInteraction.subagentMode.enabled"
          data-agent-id="settings.agent.subagent.on"
          :class="{ 'is-active': agentInteraction.subagentMode.enabled }"
            :disabled="savingAgentInteraction"
            @click="setSubagentModeEnabled(true)"
          >
            开启
          </button>
        </div>

        <component
          :is="subagentCardExpanded ? ChevronDown : ChevronRight"
          :size="16"
          aria-hidden="true"
          class="subagent-mode-card__chevron"
        />
      </div>

      <div v-if="subagentCardExpanded" :id="subagentDetailsId" class="subagent-mode-card__details">
        <div class="settings-row settings-row--nested">
          <div class="settings-row__label">Codex Subagent</div>
          <div class="ui-segmented" role="radiogroup" aria-label="Codex Subagent">
            <button
          type="button"
          role="radio"
          :aria-checked="!agentInteraction.subagentMode.codex.enabled"
          data-agent-id="settings.agent.subagent.codex.off"
          :class="{ 'is-active': !agentInteraction.subagentMode.codex.enabled }"
              :disabled="savingAgentInteraction || !agentInteraction.subagentMode.enabled"
              @click="setCodexSubagentEnabled(false)"
            >
              关闭
            </button>
            <button
          type="button"
          role="radio"
          :aria-checked="agentInteraction.subagentMode.codex.enabled"
          data-agent-id="settings.agent.subagent.codex.on"
          :class="{ 'is-active': agentInteraction.subagentMode.codex.enabled }"
              :disabled="savingAgentInteraction || !agentInteraction.subagentMode.enabled"
              @click="setCodexSubagentEnabled(true)"
            >
              开启
            </button>
          </div>
        </div>

        <div class="settings-row settings-row--nested">
          <div class="settings-row__label">Claude Subagent</div>
          <div class="ui-segmented" role="radiogroup" aria-label="Claude Subagent">
            <button
          type="button"
          role="radio"
          :aria-checked="!agentInteraction.subagentMode.claude.enabled"
          data-agent-id="settings.agent.subagent.claude.off"
          :class="{ 'is-active': !agentInteraction.subagentMode.claude.enabled }"
              :disabled="savingAgentInteraction || !agentInteraction.subagentMode.enabled"
              @click="setClaudeSubagentField('enabled', false)"
            >
              关闭
            </button>
            <button
          type="button"
          role="radio"
          :aria-checked="agentInteraction.subagentMode.claude.enabled"
          data-agent-id="settings.agent.subagent.claude.on"
          :class="{ 'is-active': agentInteraction.subagentMode.claude.enabled }"
              :disabled="savingAgentInteraction || !agentInteraction.subagentMode.enabled"
              @click="setClaudeSubagentField('enabled', true)"
            >
              开启
            </button>
          </div>
        </div>

        <div class="settings-row settings-row--nested">
          <div class="settings-row__label">Claude 转发子代理文本</div>
          <div class="ui-segmented" role="radiogroup" aria-label="Claude 转发子代理文本">
            <button
          type="button"
          role="radio"
          :aria-checked="!agentInteraction.subagentMode.claude.forwardSubagentText"
          data-agent-id="settings.agent.subagent.claude.forward-text.off"
          :class="{ 'is-active': !agentInteraction.subagentMode.claude.forwardSubagentText }"
              :disabled="savingAgentInteraction || !agentInteraction.subagentMode.enabled || !agentInteraction.subagentMode.claude.enabled"
              @click="setClaudeSubagentField('forwardSubagentText', false)"
            >
              关闭
            </button>
            <button
          type="button"
          role="radio"
          :aria-checked="agentInteraction.subagentMode.claude.forwardSubagentText"
          data-agent-id="settings.agent.subagent.claude.forward-text.on"
          :class="{ 'is-active': agentInteraction.subagentMode.claude.forwardSubagentText }"
              :disabled="savingAgentInteraction || !agentInteraction.subagentMode.enabled || !agentInteraction.subagentMode.claude.enabled"
              @click="setClaudeSubagentField('forwardSubagentText', true)"
            >
              开启
            </button>
          </div>
        </div>

        <div class="settings-row settings-row--nested">
          <div class="settings-row__label">Claude 进度摘要</div>
          <div class="ui-segmented" role="radiogroup" aria-label="Claude 进度摘要">
            <button
          type="button"
          role="radio"
          :aria-checked="!agentInteraction.subagentMode.claude.agentProgressSummaries"
          data-agent-id="settings.agent.subagent.claude.progress-summary.off"
          :class="{ 'is-active': !agentInteraction.subagentMode.claude.agentProgressSummaries }"
              :disabled="savingAgentInteraction || !agentInteraction.subagentMode.enabled || !agentInteraction.subagentMode.claude.enabled"
              @click="setClaudeSubagentField('agentProgressSummaries', false)"
            >
              关闭
            </button>
            <button
          type="button"
          role="radio"
          :aria-checked="agentInteraction.subagentMode.claude.agentProgressSummaries"
          data-agent-id="settings.agent.subagent.claude.progress-summary.on"
          :class="{ 'is-active': agentInteraction.subagentMode.claude.agentProgressSummaries }"
              :disabled="savingAgentInteraction || !agentInteraction.subagentMode.enabled || !agentInteraction.subagentMode.claude.enabled"
              @click="setClaudeSubagentField('agentProgressSummaries', true)"
            >
              开启
            </button>
          </div>
        </div>
      </div>
    </section>

    <Suspense v-if="subagentCatalogReady">
      <SubagentCatalogSection />
    </Suspense>
    <section v-else class="card subagent-section subagent-section--placeholder" aria-busy="true">
      <div class="subagent-section__header">
        <div class="subagent-section__title">
          <span>自定义 Agent</span>
        </div>
      </div>
      <div class="subagent-empty">首屏加载后补齐自定义 Agent 目录…</div>
    </section>

    <div v-if="agentInteractionError" class="conn-banner conn-banner--err">
      <AlertTriangle :size="16" aria-hidden="true" />
      <div>
        <div class="conn-banner__title">Agent 交互</div>
        <div class="conn-banner__hint">{{ agentInteractionError }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.agent-settings-stack {
  display: grid;
  gap: 12px;
}

.agent-settings-card {
  display: grid;
  gap: 12px;
}

.subagent-mode-card {
  overflow: hidden;
}

.auto-turn-card {
  display: grid;
  gap: 12px;
}

.auto-turn-card__head {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: start;
  gap: 14px;
}

.auto-turn-card__hint {
  margin: 4px 0 0;
  color: var(--text-secondary, rgba(255, 255, 255, 0.6));
  font-size: 13px;
}

.auto-turn-card__grid {
  display: grid;
  gap: 4px;
}

.permission-toggle-list {
  display: grid;
  gap: 8px;
}

.permission-toggle-item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  padding: 10px 0;
  border-top: 1px solid var(--ui-border, rgba(255, 255, 255, 0.08));
}

.permission-toggle-item:first-child {
  border-top: 0;
}

.permission-toggle-item__content {
  min-width: 0;
}

.permission-toggle-item__hint {
  margin: 4px 0 0;
  color: var(--text-secondary, rgba(255, 255, 255, 0.6));
  font-size: 13px;
}

.permission-toggle-item__badge {
  margin-left: 8px;
  color: var(--text-secondary, rgba(255, 255, 255, 0.6));
  font-size: 12px;
  font-weight: 500;
}

.permission-toggle-item__locked {
  min-width: 74px;
  justify-content: center;
}

.main-agent-prompt-panel {
  display: grid;
  gap: 10px;
  padding-top: 12px;
  border-top: 1px solid var(--ui-border, rgba(255, 255, 255, 0.08));
}

.main-agent-prompt-panel__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.main-agent-prompt-panel__meta {
  color: var(--text-secondary, rgba(255, 255, 255, 0.6));
  font-size: 12px;
}

.main-agent-prompt-panel__preview {
  max-height: 240px;
  margin: 0;
  padding: 10px 12px;
  overflow: auto;
  border: 1px solid var(--ui-border, rgba(255, 255, 255, 0.08));
  border-radius: 8px;
  background: var(--bg-elev-2, rgba(255, 255, 255, 0.02));
  color: var(--text-secondary, rgba(255, 255, 255, 0.68));
  font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, Consolas, monospace);
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
}

.main-agent-prompt-panel__editor {
  display: grid;
  gap: 8px;
}

.main-agent-prompt-panel__textarea {
  min-height: 156px;
  resize: vertical;
  font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, Consolas, monospace);
  font-size: 12px;
  line-height: 1.55;
}

.main-agent-prompt-panel__save {
  justify-self: start;
}

.subagent-mode-card__header {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 12px;
  padding: 0;
  cursor: pointer;
}

.subagent-mode-card__header[aria-disabled="true"] {
  cursor: default;
}

.subagent-mode-card__header:focus-visible {
  outline: 2px solid rgba(255, 255, 255, 0.18);
  outline-offset: -2px;
  border-radius: 14px;
}

.subagent-mode-card__summary {
  min-width: 0;
}

.subagent-mode-card__hint {
  margin: 4px 0 0;
  color: var(--text-secondary, rgba(255, 255, 255, 0.6));
  font-size: 13px;
}

.subagent-mode-card__switch {
  justify-self: end;
}

.subagent-mode-card__chevron {
  color: var(--text-secondary, rgba(255, 255, 255, 0.6));
}

.subagent-mode-card__details {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--ui-border, rgba(255, 255, 255, 0.08));
}

.settings-row--nested {
  padding-left: 12px;
}

.subagent-mode-card__details .settings-row--nested {
  padding-left: 0;
}

.subagent-section--placeholder {
  opacity: 0.78;
}
</style>

