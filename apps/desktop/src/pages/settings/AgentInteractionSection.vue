<script setup lang="ts">
import { onMounted, ref } from "vue";
import { AlertTriangle, Sparkles } from "lucide-vue-next";
import type {
  AgentInteractionSettings,
  AgentRuntimeChannel,
} from "@lilia/contracts";
import {
  useAgentInteractionSettings,
} from "../../composables/useAgentInteractionSettings";

const agentInteractionStore = useAgentInteractionSettings();
const agentInteraction = agentInteractionStore.settings;
const savingAgentInteraction = ref(false);
const agentInteractionError = ref<string | null>(null);

const runtimeChannelOptions: Array<{ value: AgentRuntimeChannel; label: string }> = [
  { value: "builtin", label: "内置" },
  { value: "mutsuki_core", label: "MutsukiCore" },
];

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

async function setRuntimeChannel(agentRuntimeChannel: AgentRuntimeChannel) {
  await setAgentInteraction({ agentRuntimeChannel });
}

async function setAgentInteraction(patch: Partial<AgentInteractionSettings>) {
  const next = { ...agentInteraction.value, ...patch };
  if (
    next.nonInterruptMode === agentInteraction.value.nonInterruptMode &&
    next.debug === agentInteraction.value.debug &&
    next.agentRuntimeChannel === agentInteraction.value.agentRuntimeChannel
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
      <div class="settings-row__label">运行时通道</div>
      <div class="settings-row__control settings-row__control--loose runtime-channel-control">
        <div class="ui-segmented" role="radiogroup" aria-label="运行时通道">
          <button
            v-for="option in runtimeChannelOptions"
            :key="option.value"
            type="button"
            role="radio"
            :aria-checked="agentInteraction.agentRuntimeChannel === option.value"
            :class="{ 'is-active': agentInteraction.agentRuntimeChannel === option.value }"
            :disabled="savingAgentInteraction"
            @click="setRuntimeChannel(option.value)"
          >
            {{ option.label }}
          </button>
        </div>
        <span class="settings-row__status-text">
          MutsukiCore 是实验性本地运行时通道；当前不包含远程任务执行和移动端访问。
        </span>
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
.runtime-channel-control {
  max-width: min(520px, 100%);
}

.runtime-channel-control .settings-row__status-text {
  white-space: normal;
  text-align: right;
}
</style>
