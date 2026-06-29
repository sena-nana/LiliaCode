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

type PluginSettingsSection = "skills" | "packages" | "hooks" | "mcp";

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
  "remote-control": lazySettingsSection("remote-control", () => import("./settings/RemoteControlSection.vue")),
  assistant: lazySettingsSection("assistant", () => import("./settings/AssistantAISection.vue")),
  "model-config": lazySettingsSection("model-config", () => import("./settings/ModelConfigurationSection.vue")),
  agent: lazySettingsSection("agent", () => import("./settings/AgentInteractionSection.vue")),
  quota: lazySettingsSection("quota", () => import("./settings/QuotaUsageSection.vue")),
  "plugin-skills": lazySettingsSection("plugin-skills", () => import("./Plugins.vue")),
  "plugin-packages": lazySettingsSection("plugin-packages", () => import("./Plugins.vue")),
  "plugin-hooks": lazySettingsSection("plugin-hooks", () => import("./Plugins.vue")),
  "plugin-mcp": lazySettingsSection("plugin-mcp", () => import("./Plugins.vue")),
  import: lazySettingsSection("import", () => import("./ConversationImport.vue")),
  project: lazySettingsSection("project", () => import("./settings/ProjectPreferencesSection.vue")),
  about: lazySettingsSection("about", () => import("./settings/AboutSection.vue")),
};

function pluginSectionFor(tab: SettingsTabKey): PluginSettingsSection | null {
  if (tab === "plugin-skills") return "skills";
  if (tab === "plugin-packages") return "packages";
  if (tab === "plugin-hooks") return "hooks";
  if (tab === "plugin-mcp") return "mcp";
  return null;
}

const route = useRoute();
const activeTab = computed(() => normalizeSettingsTab(route.query.tab));
const activeTabSection = computed(() => SETTINGS_SECTIONS[activeTab.value]);
const activePluginSection = computed(() => pluginSectionFor(activeTab.value));
const activeTabSectionProps = computed(() => (
  activePluginSection.value ? { section: activePluginSection.value } : {}
));
const isFullPageSection = computed(() =>
  activePluginSection.value !== null || activeTab.value === "import",
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
  <component
    v-if="isFullPageSection"
    :is="activeTabSection"
    v-bind="activeTabSectionProps"
    data-agent-id="settings.full-page-section"
  />
  <section v-else class="settings-page" :data-agent-id="`settings.page.${activeTab}`">
    <component :is="activeTabSection" />
  </section>
</template>

