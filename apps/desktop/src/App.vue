<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { RouterView, useRouter } from "vue-router";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ContextMenuHost from "./components/ContextMenuHost.vue";
import { installAgentInteractionBridge } from "./composables/useAgentInteractionBridge";
import { installConversationActivityBridge } from "./composables/useConversationActivity";

let unlistenInteraction: (() => void) | null = null;
let unlistenConversationActivity: (() => void) | null = null;
let unlistenMainNavigate: (() => void) | null = null;
let unlistenPopupNavigate: (() => void) | null = null;
let disposed = false;

const router = useRouter();
const appWindow = getCurrentWindow();
const isMainWindow = appWindow.label === "main";
const isPopupWindow = appWindow.label.startsWith("popup-");

function keepCleanup(cleanup: () => void): (() => void) | null {
  if (disposed) {
    cleanup();
    return null;
  }
  return cleanup;
}

onMounted(async () => {
  unlistenInteraction = keepCleanup(await installAgentInteractionBridge());
  if (!unlistenInteraction) return;

  unlistenConversationActivity = keepCleanup(await installConversationActivityBridge());
  if (!unlistenConversationActivity) return;

  if (isMainWindow) {
    const mainNavigateCleanup = await listen<{ route: string }>("lilia:main:navigate", (event) => {
      const route = event.payload.route;
      if (typeof route === "string" && route.startsWith("/")) {
        void router.push(route);
      }
    });
    unlistenMainNavigate = keepCleanup(mainNavigateCleanup);
    if (!unlistenMainNavigate) return;
  }

  if (isPopupWindow) {
    const popupNavigateCleanup = await listen<{ route: string }>("lilia:popup:navigate", (event) => {
      const route = event.payload.route;
      if (typeof route === "string" && route.startsWith("/popup/")) {
        void router.replace(route);
      }
    });
    unlistenPopupNavigate = keepCleanup(popupNavigateCleanup);
  }
});

onBeforeUnmount(() => {
  disposed = true;
  unlistenInteraction?.();
  unlistenConversationActivity?.();
  unlistenMainNavigate?.();
  unlistenPopupNavigate?.();
  unlistenInteraction = null;
  unlistenConversationActivity = null;
  unlistenMainNavigate = null;
  unlistenPopupNavigate = null;
});
</script>

<template>
  <RouterView />
  <ContextMenuHost />
</template>
