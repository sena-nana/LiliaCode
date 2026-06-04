<script setup lang="ts">
import AppearanceSection from "./settings/AppearanceSection.vue";
import PopupWindowSection from "./settings/PopupWindowSection.vue";
import ProviderConnectionSection from "./settings/ProviderConnectionSection.vue";
import AssistantAISection from "./settings/AssistantAISection.vue";
import AgentInteractionSection from "./settings/AgentInteractionSection.vue";
import ProjectPreferencesSection from "./settings/ProjectPreferencesSection.vue";
import AboutSection from "./settings/AboutSection.vue";
</script>

<template>
  <section>
    <div class="page-header">
      <div>
        <h1>设置</h1>
        <p>对话统一经 CC-Switch 本地代理转发。</p>
      </div>
    </div>

    <AppearanceSection />
    <PopupWindowSection />
    <ProviderConnectionSection />
    <AssistantAISection />
    <AgentInteractionSection />
    <ProjectPreferencesSection />
    <AboutSection />
  </section>
</template>
