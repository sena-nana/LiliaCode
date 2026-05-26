<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { RouterView } from "vue-router";
import ContextMenuHost from "./components/ContextMenuHost.vue";
import AskUserHost from "./components/AskUserHost.vue";
import { installToolConsentBridge } from "./composables/useToolConsentBridge";

let unlistenConsent: (() => void) | null = null;

onMounted(async () => {
  unlistenConsent = await installToolConsentBridge();
});

onBeforeUnmount(() => {
  unlistenConsent?.();
  unlistenConsent = null;
});
</script>

<template>
  <RouterView />
  <ContextMenuHost />
  <AskUserHost />
</template>
