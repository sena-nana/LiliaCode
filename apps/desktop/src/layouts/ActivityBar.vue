<script setup lang="ts">
import { FolderKanban, Settings } from "lucide-vue-next";

interface Props {
  active: "projects" | "settings";
}
defineProps<Props>();
defineEmits<{ navigate: ["projects" | "settings"] }>();
</script>

<template>
  <aside class="activity-bar">
    <div class="activity-bar__top">
      <button
        type="button"
        class="activity-bar__item"
        :class="{ 'is-active': active === 'projects' }"
        aria-label="项目"
        title="项目"
        @click="$emit('navigate', 'projects')"
      >
        <FolderKanban :size="20" aria-hidden="true" />
      </button>
    </div>
    <div class="activity-bar__bottom">
      <button
        type="button"
        class="activity-bar__item"
        :class="{ 'is-active': active === 'settings' }"
        aria-label="设置"
        title="设置"
        @click="$emit('navigate', 'settings')"
      >
        <Settings :size="20" aria-hidden="true" />
      </button>
    </div>
  </aside>
</template>
