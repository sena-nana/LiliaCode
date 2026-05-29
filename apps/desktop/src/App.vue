<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { RouterView } from "vue-router";
import ContextMenuHost from "./components/ContextMenuHost.vue";
import { installAgentAskUserBridge } from "./composables/useAgentAskUserBridge";
import { installToolConsentBridge } from "./composables/useToolConsentBridge";

let unlistenConsent: (() => void) | null = null;
let unlistenAskUser: (() => void) | null = null;

onMounted(async () => {
  const [consent, askUser] = await Promise.all([
    installToolConsentBridge(),
    installAgentAskUserBridge(),
  ]);
  unlistenConsent = consent;
  unlistenAskUser = askUser;
});

onBeforeUnmount(() => {
  unlistenConsent?.();
  unlistenAskUser?.();
  unlistenConsent = null;
  unlistenAskUser = null;
});
</script>

<template>
  <RouterView />
  <ContextMenuHost />
</template>
