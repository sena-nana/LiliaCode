<script setup lang="ts">
import { List, Moon, PanelsTopLeft, Sun } from "lucide-vue-next";
import { useTheme } from "../../composables/useTheme";
import { useSidebarDisplayMode } from "../../composables/useSidebarDisplayMode";

const { theme, setTheme } = useTheme();
const { sidebarDisplayMode, setSidebarDisplayMode } = useSidebarDisplayMode();
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
      <div class="settings-row__label">侧边栏样式</div>
      <div class="ui-segmented" role="radiogroup" aria-label="侧边栏样式">
        <button
          type="button"
          role="radio"
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
