<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { ArrowLeft } from "lucide-vue-next";
import {
  SETTINGS_TABS,
  normalizeSettingsTab,
  type SettingsTabKey,
} from "../pages/settings/settingsTabs";

const props = defineProps<{
  returnTo?: string | null;
}>();

const route = useRoute();
const router = useRouter();

const activeTab = computed(() => normalizeSettingsTab(route.query.tab));

function goBack() {
  router.push(props.returnTo || "/");
}

function openTab(tab: SettingsTabKey) {
  router.push({
    path: "/settings",
    query: { tab },
  });
}
</script>

<template>
  <aside class="secondary-panel settings-sidebar" aria-label="设置分类">
    <div class="settings-sidebar__head">
      <button
        type="button"
        class="settings-sidebar__back"
        aria-label="返回"
        title="返回"
        @click="goBack"
      >
        <ArrowLeft :size="15" aria-hidden="true" />
        <span>返回</span>
      </button>
    </div>

    <nav class="settings-sidebar__tabs" aria-label="设置分类">
      <button
        v-for="tab in SETTINGS_TABS"
        :key="tab.key"
        type="button"
        class="settings-sidebar__tab"
        :class="{ 'is-active': activeTab === tab.key }"
        :aria-current="activeTab === tab.key ? 'page' : undefined"
        @click="openTab(tab.key)"
      >
        <component
          :is="tab.icon"
          class="settings-sidebar__tab-icon"
          :size="15"
          aria-hidden="true"
        />
        <span class="settings-sidebar__tab-label">{{ tab.label }}</span>
      </button>
    </nav>
  </aside>
</template>
