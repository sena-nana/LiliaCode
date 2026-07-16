<script setup lang="ts">
import { computed } from "vue";
import { RouterView, useRoute } from "vue-router";
import LiliaPopupShell from "@lilia/ui/layouts/PopupShell";
import PopupTitleBar from "../components/PopupTitleBar.vue";

const route = useRoute();
const isStatusFloat = computed(() => route.path === "/popup/status");
</script>

<template>
  <LiliaPopupShell :status="isStatusFloat">
    <template v-if="!isStatusFloat" #titlebar>
      <PopupTitleBar />
    </template>
    <RouterView />
  </LiliaPopupShell>
</template>
