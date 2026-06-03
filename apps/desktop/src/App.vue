<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { RouterView, useRouter } from "vue-router";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ContextMenuHost from "./components/ContextMenuHost.vue";
import { installAgentAskUserBridge } from "./composables/useAgentAskUserBridge";
import { installToolConsentBridge } from "./composables/useToolConsentBridge";

let unlistenConsent: (() => void) | null = null;
let unlistenAskUser: (() => void) | null = null;
let unlistenMainNavigate: (() => void) | null = null;

const router = useRouter();
const appWindow = getCurrentWindow();
const isMainWindow = appWindow.label === "main";

onMounted(async () => {
  const [consent, askUser] = await Promise.all([
    installToolConsentBridge(),
    installAgentAskUserBridge(),
  ]);
  unlistenConsent = consent;
  unlistenAskUser = askUser;

  if (isMainWindow) {
    unlistenMainNavigate = await listen<{ route: string }>("lilia:main:navigate", (event) => {
      const route = event.payload.route;
      if (typeof route === "string" && route.startsWith("/")) {
        void router.push(route);
      }
    });
  }
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
