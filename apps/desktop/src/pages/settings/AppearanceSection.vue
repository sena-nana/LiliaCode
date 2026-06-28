<script setup lang="ts">
import { List, Moon, PanelsTopLeft, Radius, SquareRoundCorner, Sun } from "@lucide/vue";
import {
  CORNER_RADIUS_MAX,
  CORNER_RADIUS_MIN,
  useCornerStyle,
} from "../../composables/useCornerStyle";
import { useTheme } from "../../composables/useTheme";
import { useSidebarDisplayMode } from "../../composables/useSidebarDisplayMode";

const { theme, setTheme } = useTheme();
const { cornerStyle, cornerRadius, setCornerRadius, setCornerStyle } = useCornerStyle();
const { sidebarDisplayMode, setSidebarDisplayMode } = useSidebarDisplayMode();

function onCornerRadiusInput(event: Event) {
  setCornerRadius(Number((event.target as HTMLInputElement).value));
}
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
  </div>
</template>

