<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { RouterView, useRouter } from "vue-router";
import { listen } from "@tauri-apps/api/event";
import ContextMenuHost from "./components/ContextMenuHost.vue";
import { installAgentAskUserBridge } from "./composables/useAgentAskUserBridge";
import { installToolConsentBridge } from "./composables/useToolConsentBridge";

let unlistenConsent: (() => void) | null = null;
let unlistenAskUser: (() => void) | null = null;
let unlistenMainNavigate: (() => void) | null = null;

const router = useRouter();

onMounted(async () => {
  const [consent, askUser, mainNavigate] = await Promise.all([
    installToolConsentBridge(),
    installAgentAskUserBridge(),
    listen<{ route: string }>("lilia:main:navigate", (event) => {
      const route = event.payload.route;
      if (typeof route === "string" && route.startsWith("/")) {
        void router.push(route);
      }
    }),
  ]);
  unlistenConsent = consent;
  unlistenAskUser = askUser;
  unlistenMainNavigate = mainNavigate;
});

onBeforeUnmount(() => {
  unlistenConsent?.();
  unlistenAskUser?.();
  unlistenMainNavigate?.();
  unlistenConsent = null;
  unlistenAskUser = null;
  unlistenMainNavigate = null;
});
</script>

<template>
  <RouterView />
  <ContextMenuHost />
</template>
