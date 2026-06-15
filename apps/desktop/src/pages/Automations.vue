<script setup lang="ts">
import { defineAsyncComponent } from "vue";

const AutomationWorkspacePage = defineAsyncComponent(() =>
  import("./automations/AutomationWorkspacePage.vue"),
);
</script>

<template>
  <AutomationWorkspacePage />
</template>
