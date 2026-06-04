<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { RouterView, useRouter } from "vue-router";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ContextMenuHost from "./components/ContextMenuHost.vue";
import { installAgentInteractionBridge } from "./composables/useAgentInteractionBridge";

let unlistenInteraction: (() => void) | null = null;
let unlistenMainNavigate: (() => void) | null = null;
let unlistenPopupNavigate: (() => void) | null = null;

const router = useRouter();
const appWindow = getCurrentWindow();
const isMainWindow = appWindow.label === "main";
const isPopupWindow = appWindow.label.startsWith("popup-");

onMounted(async () => {
  unlistenInteraction = await installAgentInteractionBridge();

  if (isMainWindow) {
    unlistenMainNavigate = await listen<{ route: string }>("lilia:main:navigate", (event) => {
      const route = event.payload.route;
      if (typeof route === "string" && route.startsWith("/")) {
        void router.push(route);
      }
    });
  }

  if (isPopupWindow) {
    unlistenPopupNavigate = await listen<{ route: string }>("lilia:popup:navigate", (event) => {
      const route = event.payload.route;
      if (typeof route === "string" && route.startsWith("/popup/")) {
        void router.replace(route);
      }
    });
  }
});

onBeforeUnmount(() => {
  unlistenInteraction?.();
  unlistenMainNavigate?.();
  unlistenPopupNavigate?.();
  unlistenInteraction = null;
  unlistenMainNavigate = null;
  unlistenPopupNavigate = null;
});
</script>

<template>
  <RouterView />
  <ContextMenuHost />
</template>
