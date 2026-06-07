<script setup lang="ts">
import { computed, type Component } from "vue";
import { useRoute } from "vue-router";
import AppearanceSection from "./settings/AppearanceSection.vue";
import PopupWindowSection from "./settings/PopupWindowSection.vue";
import ProviderConnectionSection from "./settings/ProviderConnectionSection.vue";
import AssistantAISection from "./settings/AssistantAISection.vue";
import AgentInteractionSection from "./settings/AgentInteractionSection.vue";
import ProjectPreferencesSection from "./settings/ProjectPreferencesSection.vue";
import AboutSection from "./settings/AboutSection.vue";
import {
  SETTINGS_TABS,
  normalizeSettingsTab,
  type SettingsTabKey,
} from "./settings/settingsTabs";

const SETTINGS_SECTIONS: Record<SettingsTabKey, Component> = {
  appearance: AppearanceSection,
  window: PopupWindowSection,
  providers: ProviderConnectionSection,
  assistant: AssistantAISection,
  agent: AgentInteractionSection,
  project: ProjectPreferencesSection,
  about: AboutSection,
};

const route = useRoute();
const activeTab = computed(() => normalizeSettingsTab(route.query.tab));
const activeTabSection = computed(() => SETTINGS_SECTIONS[activeTab.value]);
const activeTabLabel = computed(
  () => SETTINGS_TABS.find((tab) => tab.key === activeTab.value)?.label ?? "设置",
);
</script>

<template>
  <section class="settings-page">
    <div class="page-header">
      <div>
        <h1>{{ activeTabLabel }}</h1>
      </div>
    </div>

    <component :is="activeTabSection" />
  </section>
</template>
