<script setup lang="ts">
import { computed, type Component } from "vue";
import { useRoute } from "vue-router";
import AppearanceSection from "./settings/AppearanceSection.vue";
import PopupWindowSection from "./settings/PopupWindowSection.vue";
import ProviderConnectionSection from "./settings/ProviderConnectionSection.vue";
import AssistantAISection from "./settings/AssistantAISection.vue";
import AgentInteractionSection from "./settings/AgentInteractionSection.vue";
import QuotaUsageSection from "./settings/QuotaUsageSection.vue";
import ProjectPreferencesSection from "./settings/ProjectPreferencesSection.vue";
import AboutSection from "./settings/AboutSection.vue";
import Plugins from "./Plugins.vue";
import ConversationImport from "./ConversationImport.vue";
import {
  normalizeSettingsTab,
  type SettingsTabKey,
} from "./settings/settingsTabs";

const SETTINGS_SECTIONS: Record<SettingsTabKey, Component> = {
  appearance: AppearanceSection,
  window: PopupWindowSection,
  providers: ProviderConnectionSection,
  assistant: AssistantAISection,
  agent: AgentInteractionSection,
  quota: QuotaUsageSection,
  plugins: Plugins,
  import: ConversationImport,
  project: ProjectPreferencesSection,
  about: AboutSection,
};

const route = useRoute();
const activeTab = computed(() => normalizeSettingsTab(route.query.tab));
const activeTabSection = computed(() => SETTINGS_SECTIONS[activeTab.value]);
const isFullPageSection = computed(() =>
  activeTab.value === "plugins" || activeTab.value === "import",
);
</script>

<template>
  <component v-if="isFullPageSection" :is="activeTabSection" />
  <section v-else class="settings-page">
    <component :is="activeTabSection" />
  </section>
</template>
