<script setup lang="ts">
import { computed } from "vue";
import { RouterView, useRoute } from "vue-router";
import PopupTitleBar from "../components/PopupTitleBar.vue";

const route = useRoute();
const isStatusFloat = computed(() => route.path === "/popup/status");
</script>

<template>
  <div class="popup-shell" :class="{ 'popup-shell--status': isStatusFloat }">
    <PopupTitleBar v-if="!isStatusFloat" />
    <main class="popup-shell__main">
      <RouterView />
    </main>
  </div>
</template>
