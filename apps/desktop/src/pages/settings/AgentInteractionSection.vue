<script setup lang="ts">
import { onMounted, ref } from "vue";
import { AlertTriangle, Sparkles } from "lucide-vue-next";
import type { AgentInteractionSettings } from "@lilia/contracts";
import { useAgentInteractionSettings } from "../../composables/useAgentInteractionSettings";

const agentInteractionStore = useAgentInteractionSettings();
const agentInteraction = agentInteractionStore.settings;
const savingAgentInteraction = ref(false);
const agentInteractionError = ref<string | null>(null);

async function loadAgentInteraction() {
  try {
    await agentInteractionStore.load();
  } catch (err) {
    agentInteractionError.value = `读取 Agent 交互设置失败：${String(err)}`;
  }
}

async function setNonInterruptMode(nonInterruptMode: boolean) {
  await setAgentInteraction({ nonInterruptMode });
}

async function setDebugMode(debug: boolean) {
  await setAgentInteraction({ debug });
}

async function setAgentInteraction(patch: Partial<AgentInteractionSettings>) {
  const next = { ...agentInteraction.value, ...patch };
  if (
    next.nonInterruptMode === agentInteraction.value.nonInterruptMode &&
    next.debug === agentInteraction.value.debug
  ) {
    return;
  }
  savingAgentInteraction.value = true;
  agentInteractionError.value = null;
  try {
    await agentInteractionStore.update(patch);
  } catch (err) {
    agentInteractionError.value = `保存 Agent 交互设置失败：${String(err)}`;
  } finally {
    savingAgentInteraction.value = false;
  }
}

onMounted(loadAgentInteraction);
</script>

<template>
  <div class="card">
    <h2>
      <span class="card-h2__title">
        <Sparkles :size="14" aria-hidden="true" />
        Agent 交互
      </span>
    </h2>

    <div class="settings-row">
      <div class="settings-row__label">
        <div>非打断模式</div>
        <div class="settings-row__hint">权限、提问和计划确认留在时间线卡片中处理。</div>
      </div>
      <div class="segmented" role="radiogroup" aria-label="非打断模式">
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
      <div class="settings-row__label">
        <div>Debug 面板</div>
        <div class="settings-row__hint">在对话侧栏加入临时事件注入面板。</div>
      </div>
      <div class="segmented" role="radiogroup" aria-label="Debug 面板">
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

    <div v-if="agentInteractionError" class="conn-banner conn-banner--err">
      <AlertTriangle :size="16" aria-hidden="true" />
      <div>
        <div class="conn-banner__title">Agent 交互</div>
        <div class="conn-banner__hint">{{ agentInteractionError }}</div>
      </div>
    </div>
  </div>
</template>
