<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, watch, type Component } from "vue";
import { useRoute } from "vue-router";
import {
  normalizeSettingsTab,
  type SettingsTabKey,
} from "./settings/settingsTabs";
import {
  beginPerfStage,
  installPerfObservers,
  measurePerfAsync,
  scheduleAfterPaint,
} from "../utils/perf";
import { createLazyLoadState } from "../utils/lazyLoadState";

function lazySettingsSection(
  tab: SettingsTabKey,
  loader: () => Promise<{ default: Component }>,
): Component {
  const state = createLazyLoadState(() =>
    measurePerfAsync(
      "settings.tab.load",
      async () => (await loader()).default,
      { detail: tab },
    )
  );
  return defineAsyncComponent({
    suspensible: false,
    loader: () => state.load(),
  });
}

const SETTINGS_SECTIONS: Record<SettingsTabKey, Component> = {
  appearance: lazySettingsSection("appearance", () => import("./settings/AppearanceSection.vue")),
  window: lazySettingsSection("window", () => import("./settings/PopupWindowSection.vue")),
  providers: lazySettingsSection("providers", () => import("./settings/ProviderConnectionSection.vue")),
  assistant: lazySettingsSection("assistant", () => import("./settings/AssistantAISection.vue")),
  agent: lazySettingsSection("agent", () => import("./settings/AgentInteractionSection.vue")),
  quota: lazySettingsSection("quota", () => import("./settings/QuotaUsageSection.vue")),
  plugins: lazySettingsSection("plugins", () => import("./Plugins.vue")),
  import: lazySettingsSection("import", () => import("./ConversationImport.vue")),
  project: lazySettingsSection("project", () => import("./settings/ProjectPreferencesSection.vue")),
  about: lazySettingsSection("about", () => import("./settings/AboutSection.vue")),
};

const route = useRoute();
const activeTab = computed(() => normalizeSettingsTab(route.query.tab));
const activeTabSection = computed(() => SETTINGS_SECTIONS[activeTab.value]);
const isFullPageSection = computed(() =>
  activeTab.value === "plugins" || activeTab.value === "import",
);
let cancelTabSwitchPaintMeasure: (() => void) | null = null;

installPerfObservers();

watch(
  () => activeTab.value,
  (tab) => {
    cancelTabSwitchPaintMeasure?.();
    const stage = beginPerfStage("settings.tab.switch", { detail: tab });
    const cancelPaint = scheduleAfterPaint(() => {
      if (cancelTabSwitchPaintMeasure === cancelPaint) {
        cancelTabSwitchPaintMeasure = null;
      }
      stage.end("paint");
    });
    cancelTabSwitchPaintMeasure = cancelPaint;
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  cancelTabSwitchPaintMeasure?.();
  cancelTabSwitchPaintMeasure = null;
});
</script>

<template>
  <component v-if="isFullPageSection" :is="activeTabSection" />
  <section v-else class="settings-page">
    <component :is="activeTabSection" />
  </section>
</template>
