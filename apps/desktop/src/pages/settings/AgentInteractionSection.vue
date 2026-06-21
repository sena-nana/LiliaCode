<script setup lang="ts">
import { defineAsyncComponent, onBeforeUnmount, onMounted, ref, type Component, watch } from "vue";
import {
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  Sparkles,
} from "lucide-vue-next";
import type { AgentInteractionSettings, PermissionMode } from "@lilia/contracts";
import {
  AUTO_TURN_DECISION_PERMISSION_OPTIONS,
  PERMISSION_MODE_DISPLAY,
  PERMISSION_MODE_DISPLAY_ORDER,
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

const SubagentCatalogSection = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "settings.agent.subagents.load",
    async () => (await import("./SubagentCatalogSection.vue")).default as Component,
  ),
});

const agentInteractionStore = useAgentInteractionSettings();
const agentInteraction = agentInteractionStore.settings;
const savingAgentInteraction = ref(false);
const agentInteractionError = ref<string | null>(null);
const subagentCardExpanded = ref(false);
const subagentCatalogReady = ref(false);
const subagentDetailsId = "agent-subagent-mode-details";
let subagentCatalogIdleHandle: number | null = null;
let cancelSubagentCatalogPaint: (() => void) | null = null;
let subagentCatalogMountSeq = 0;
let disposed = false;

const permissionModeOptions: Array<{ value: PermissionMode; label: string; description: string }> =
  PERMISSION_MODE_DISPLAY_ORDER.map((value) => ({
    value,
    label: PERMISSION_MODE_DISPLAY[value].label,
    description: PERMISSION_MODE_DISPLAY[value].description,
  }));

type SubagentModePatch = Partial<Omit<AgentInteractionSettings["subagentMode"], "codex" | "claude">> & {
  codex?: Partial<AgentInteractionSettings["subagentMode"]["codex"]>;
  claude?: Partial<AgentInteractionSettings["subagentMode"]["claude"]>;
};

const autoTurnDecisionPermissionOptions = AUTO_TURN_DECISION_PERMISSION_OPTIONS;

function activePermissionDescription(): string {
  return permissionModeOptions.find((option) => option.value === agentInteraction.value.permissionMode)
    ?.description ?? permissionModeOptions[0].description;
}

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

async function setPermissionMode(permissionMode: PermissionMode) {
  await setAgentInteraction({ permissionMode });
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
      claude: { [key]: value },
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
  <div class="card">
    <h2>
      <span class="card-h2__title">
        <Sparkles :size="14" aria-hidden="true" />
        Agent 交互
      </span>
    </h2>

    <section class="permission-mode-panel" aria-label="权限行为">
      <div class="permission-mode-panel__head">
        <div>
          <div class="settings-row__label">权限行为</div>
          <p class="permission-mode-panel__hint">{{ activePermissionDescription() }}</p>
        </div>
        <div class="ui-segmented permission-mode-panel__switch" role="radiogroup" aria-label="权限行为">
          <button
            v-for="option in permissionModeOptions"
            :key="option.value"
            type="button"
            role="radio"
            :aria-checked="agentInteraction.permissionMode === option.value"
            :class="{ 'is-active': agentInteraction.permissionMode === option.value }"
            :disabled="savingAgentInteraction"
            @click="setPermissionMode(option.value)"
          >
            {{ option.label }}
          </button>
        </div>
      </div>
      <div class="permission-mode-panel__descriptions">
        <span
          v-for="option in permissionModeOptions"
          :key="`${option.value}-desc`"
          class="permission-mode-panel__description"
        >
          <strong>{{ option.label }}</strong>{{ option.description }}
        </span>
      </div>
    </section>

    <div class="settings-row">
      <div class="settings-row__label">非打断模式</div>
      <div class="ui-segmented" role="radiogroup" aria-label="非打断模式">
        <button
          type="button"
          role="radio"
          :aria-checked="!agentInteraction.nonInterruptMode"
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
          :class="{ 'is-active': agentInteraction.debug }"
          :disabled="savingAgentInteraction"
          @click="setDebugMode(true)"
        >
          开启
        </button>
      </div>
    </div>

    <section class="auto-turn-card" aria-label="自动轮次策略">
      <div class="auto-turn-card__head">
        <div>
          <div class="settings-row__label">自动轮次策略</div>
          <p class="auto-turn-card__hint">Auto 模式下，由辅助模型在轮次启动前决定本轮执行策略。</p>
        </div>
        <div class="ui-segmented" role="radiogroup" aria-label="自动轮次策略">
          <button
            type="button"
            role="radio"
            :aria-checked="!agentInteraction.autoTurnDecision.enabled"
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
      class="subagent-mode-card"
    >
      <div
        class="subagent-mode-card__header"
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
    <section v-else class="subagent-section subagent-section--placeholder" aria-busy="true">
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
.subagent-mode-card {
  margin-top: 4px;
  border: 1px solid var(--ui-border, rgba(255, 255, 255, 0.08));
  border-radius: 14px;
  background: var(--bg-elev-2, rgba(255, 255, 255, 0.02));
}

.auto-turn-card {
  display: grid;
  gap: 12px;
  margin: 10px 0 12px;
  padding: 12px 0;
  border-block: 1px solid var(--ui-border, rgba(255, 255, 255, 0.08));
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

.permission-mode-panel {
  display: grid;
  gap: 10px;
  margin: 2px 0 12px;
  padding: 12px 0;
  border-block: 1px solid var(--ui-border, rgba(255, 255, 255, 0.08));
}

.permission-mode-panel__head {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: start;
  gap: 14px;
}

.permission-mode-panel__hint {
  margin: 4px 0 0;
  color: var(--text-secondary, rgba(255, 255, 255, 0.6));
  font-size: 13px;
}

.permission-mode-panel__switch {
  justify-self: end;
}

.permission-mode-panel__descriptions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px 12px;
  color: var(--text-secondary, rgba(255, 255, 255, 0.6));
  font-size: 12px;
  line-height: 1.5;
}

.permission-mode-panel__description strong {
  margin-right: 4px;
  color: var(--text-primary, rgba(255, 255, 255, 0.82));
  font-weight: 600;
}

.subagent-mode-card__header {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 12px;
  padding: 14px 16px;
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
  padding: 0 16px 12px;
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
