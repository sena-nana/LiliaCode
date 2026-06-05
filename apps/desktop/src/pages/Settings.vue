<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import AppearanceSection from "./settings/AppearanceSection.vue";
import PopupWindowSection from "./settings/PopupWindowSection.vue";
import ProviderConnectionSection from "./settings/ProviderConnectionSection.vue";
import AssistantAISection from "./settings/AssistantAISection.vue";
import AgentInteractionSection from "./settings/AgentInteractionSection.vue";
import ProjectPreferencesSection from "./settings/ProjectPreferencesSection.vue";
import AboutSection from "./settings/AboutSection.vue";
import { normalizeSettingsTab } from "./settings/settingsTabs";

const route = useRoute();
const activeTab = computed(() => normalizeSettingsTab(route.query.tab));
</script>

<template>
  <section class="settings-page">
    <div class="page-header">
      <div>
        <h1>设置</h1>
        <p v-if="activeTab === 'appearance'">调整界面外观和弹出窗口入口。</p>
        <p v-else-if="activeTab === 'providers'">对话统一经 CC-Switch 本地代理转发。</p>
        <p v-else-if="activeTab === 'agent'">设置 Agent 默认交互和 Codex profile。</p>
        <p v-else-if="activeTab === 'project'">配置项目创建与默认偏好。</p>
        <p v-else>查看 Lilia 的基础信息。</p>
      </div>
    </div>

    <template v-if="activeTab === 'appearance'">
      <AppearanceSection />
      <PopupWindowSection />
    </template>
    <template v-else-if="activeTab === 'providers'">
      <ProviderConnectionSection />
      <AssistantAISection />
    </template>
    <AgentInteractionSection v-else-if="activeTab === 'agent'" />
    <ProjectPreferencesSection v-else-if="activeTab === 'project'" />
    <AboutSection v-else />
  </section>
</template>
