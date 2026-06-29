<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import {
  AlertTriangle,
  List,
  Moon,
  PanelsTopLeft,
  Radius,
  SquareRoundCorner,
  Sun,
} from "@lucide/vue";
import {
  CORNER_RADIUS_MAX,
  CORNER_RADIUS_MIN,
  useCornerStyle,
} from "../../composables/useCornerStyle";
import { useAgentInteractionSettings } from "../../composables/useAgentInteractionSettings";
import { useTheme } from "../../composables/useTheme";
import { useSidebarDisplayMode } from "../../composables/useSidebarDisplayMode";

const { theme, setTheme } = useTheme();
const { cornerStyle, cornerRadius, setCornerRadius, setCornerStyle } = useCornerStyle();
const { sidebarDisplayMode, setSidebarDisplayMode } = useSidebarDisplayMode();
const agentInteractionStore = useAgentInteractionSettings();
const agentInteraction = agentInteractionStore.settings;
const savingAgentInteraction = ref(false);
const agentInteractionError = ref<string | null>(null);
let disposed = false;

function onCornerRadiusInput(event: Event) {
  setCornerRadius(Number((event.target as HTMLInputElement).value));
}

async function loadAgentInteraction() {
  try {
    await agentInteractionStore.load();
  } catch (err) {
    if (disposed) return;
    agentInteractionError.value = `读取运行配置失败：${String(err)}`;
  }
}

async function setAgentInteraction(
  patch: Partial<typeof agentInteraction.value>,
) {
  if (disposed) return;
  savingAgentInteraction.value = true;
  agentInteractionError.value = null;
  try {
    await agentInteractionStore.update(patch);
  } catch (err) {
    if (!disposed) agentInteractionError.value = `保存运行配置失败：${String(err)}`;
  } finally {
    if (!disposed) savingAgentInteraction.value = false;
  }
}

onMounted(() => {
  disposed = false;
  void loadAgentInteraction();
});

onBeforeUnmount(() => {
  disposed = true;
});
</script>

<template>
  <div class="card">
    <h2>外观</h2>
    <div class="settings-row">
      <div class="settings-row__label">主题</div>
      <div class="ui-segmented" role="radiogroup" aria-label="主题">
        <button
          type="button"
          role="radio"
          data-agent-id="settings.appearance.theme.dark"
          :aria-checked="theme === 'dark'"
          :class="{ 'is-active': theme === 'dark' }"
          @click="setTheme('dark')"
        >
          <Moon :size="14" aria-hidden="true" />
          暗色
        </button>
        <button
          type="button"
          role="radio"
          data-agent-id="settings.appearance.theme.light"
          :aria-checked="theme === 'light'"
          :class="{ 'is-active': theme === 'light' }"
          @click="setTheme('light')"
        >
          <Sun :size="14" aria-hidden="true" />
          浅色
        </button>
      </div>
    </div>
    <div class="settings-row">
      <div class="settings-row__label">语言</div>
      <span class="muted">简体中文</span>
    </div>
    <div class="settings-row">
      <div class="settings-row__label">圆角</div>
      <div class="ui-segmented" role="radiogroup" aria-label="圆角">
        <button
          type="button"
          role="radio"
          data-agent-id="settings.appearance.corner.smooth"
          :aria-checked="cornerStyle === 'smooth'"
          :class="{ 'is-active': cornerStyle === 'smooth' }"
          @click="setCornerStyle('smooth')"
        >
          <SquareRoundCorner :size="14" aria-hidden="true" />
          平滑
        </button>
        <button
          type="button"
          role="radio"
          data-agent-id="settings.appearance.corner.round"
          :aria-checked="cornerStyle === 'round'"
          :class="{ 'is-active': cornerStyle === 'round' }"
          @click="setCornerStyle('round')"
        >
          <Radius :size="14" aria-hidden="true" />
          普通
        </button>
      </div>
    </div>
    <div class="settings-row">
      <div class="settings-row__label">圆角半径</div>
      <div class="settings-row__control settings-row__radius-control">
        <input
          type="range"
          data-agent-id="settings.appearance.corner-radius"
          :min="CORNER_RADIUS_MIN"
          :max="CORNER_RADIUS_MAX"
          step="1"
          :value="cornerRadius"
          aria-label="圆角半径"
          @input="onCornerRadiusInput"
        />
        <output>{{ cornerRadius }}px</output>
      </div>
    </div>
    <div class="settings-row">
      <div class="settings-row__label">侧边栏样式</div>
      <div class="ui-segmented" role="radiogroup" aria-label="侧边栏样式">
        <button
          type="button"
          role="radio"
          data-agent-id="settings.appearance.sidebar.grouped"
          :aria-checked="sidebarDisplayMode === 'grouped'"
          :class="{ 'is-active': sidebarDisplayMode === 'grouped' }"
          @click="setSidebarDisplayMode('grouped')"
        >
          <PanelsTopLeft :size="14" aria-hidden="true" />
          按项目分组
        </button>
        <button
          type="button"
          role="radio"
          data-agent-id="settings.appearance.sidebar.unified"
          :aria-checked="sidebarDisplayMode === 'unified'"
          :class="{ 'is-active': sidebarDisplayMode === 'unified' }"
          @click="setSidebarDisplayMode('unified')"
        >
          <List :size="14" aria-hidden="true" />
          统一列表
        </button>
      </div>
    </div>
    <div class="appearance-settings-group" aria-label="运行配置">
      <h3>运行配置</h3>
      <div class="settings-row">
        <div class="settings-row__label">非打断模式</div>
        <div class="ui-segmented" role="radiogroup" aria-label="非打断模式">
          <button
            type="button"
            role="radio"
            :aria-checked="!agentInteraction.nonInterruptMode"
            data-agent-id="settings.appearance.runtime.non-interrupt.off"
            :class="{ 'is-active': !agentInteraction.nonInterruptMode }"
            :disabled="savingAgentInteraction"
            @click="setAgentInteraction({ nonInterruptMode: false })"
          >
            关闭
          </button>
          <button
            type="button"
            role="radio"
            :aria-checked="agentInteraction.nonInterruptMode"
            data-agent-id="settings.appearance.runtime.non-interrupt.on"
            :class="{ 'is-active': agentInteraction.nonInterruptMode }"
            :disabled="savingAgentInteraction"
            @click="setAgentInteraction({ nonInterruptMode: true })"
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
            data-agent-id="settings.appearance.runtime.debug.off"
            :class="{ 'is-active': !agentInteraction.debug }"
            :disabled="savingAgentInteraction"
            @click="setAgentInteraction({ debug: false })"
          >
            关闭
          </button>
          <button
            type="button"
            role="radio"
            :aria-checked="agentInteraction.debug"
            data-agent-id="settings.appearance.runtime.debug.on"
            :class="{ 'is-active': agentInteraction.debug }"
            :disabled="savingAgentInteraction"
            @click="setAgentInteraction({ debug: true })"
          >
            开启
          </button>
        </div>
      </div>
      <div v-if="agentInteractionError" class="conn-banner conn-banner--err">
        <AlertTriangle :size="16" aria-hidden="true" />
        <div>
          <div class="conn-banner__title">运行配置</div>
          <div class="conn-banner__hint">{{ agentInteractionError }}</div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.appearance-settings-group {
  display: grid;
  gap: 12px;
  margin-top: 14px;
  padding-top: 14px;
  border-top: 1px solid var(--ui-border, rgba(255, 255, 255, 0.08));
}

.appearance-settings-group h3 {
  margin: 0;
  color: var(--text-primary, rgba(255, 255, 255, 0.92));
  font-size: 14px;
  font-weight: 650;
}
</style>

