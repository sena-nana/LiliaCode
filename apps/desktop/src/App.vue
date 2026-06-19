<script setup lang="ts">
import { defineAsyncComponent, onBeforeUnmount, onMounted } from "vue";
import { RouterView, useRouter } from "vue-router";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  consumePendingCliProjectOpen,
  openCliProject,
  type CliProjectOpenPayload,
} from "./services/cliProjectOpen";
import { measurePerfAsync, scheduleAfterPaint } from "./utils/perf";

const ContextMenuHost = defineAsyncComponent({
  loader: () => measurePerfAsync(
    "app.context-menu-host.load",
    async () => (await import("./components/ContextMenuHost.vue")).default,
    { detail: "ContextMenuHost" },
  ),
  suspensible: false,
});

let unlistenInteraction: (() => void) | null = null;
let unlistenConversationActivity: (() => void) | null = null;
let unlistenMainNavigate: (() => void) | null = null;
let unlistenCliProjectOpen: (() => void) | null = null;
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

function handleCliProjectOpen(payload: CliProjectOpenPayload, source: string) {
  void openCliProject(payload, router).catch((err) => {
    console.error(`[liliacode] open ${source} project failed`, err);
  });
}

async function installDeferredBridges() {
  const [{ installAgentInteractionBridge }, { installConversationActivityBridge }] =
    await measurePerfAsync(
      "app.bridge.import",
      () => Promise.all([
        import("./composables/useAgentInteractionBridge"),
        import("./composables/useConversationActivity"),
      ]),
      { detail: appWindow.label },
    );
  if (disposed) return;

  unlistenInteraction = keepCleanup(await measurePerfAsync(
    "app.bridge.agent.install",
    () => installAgentInteractionBridge(),
    { detail: appWindow.label },
  ));
  if (!unlistenInteraction) return;

  unlistenConversationActivity = keepCleanup(await measurePerfAsync(
    "app.bridge.activity.install",
    () => installConversationActivityBridge(),
    { detail: appWindow.label },
  ));
}

onMounted(async () => {
  scheduleAfterPaint(() => {
    if (disposed) return;
    void installDeferredBridges().catch((err) => {
      console.error("[app] install deferred bridges failed", err);
    });
  });

  if (isMainWindow) {
    const mainNavigateCleanup = await listen<{ route: string }>("lilia:main:navigate", (event) => {
      const route = event.payload.route;
      if (typeof route === "string" && route.startsWith("/")) {
        void router.push(route);
      }
    });
    unlistenMainNavigate = keepCleanup(mainNavigateCleanup);
    if (!unlistenMainNavigate) return;

    const cliProjectOpenCleanup = await listen<CliProjectOpenPayload>(
      "lilia:cli-project-open",
      (event) => {
        handleCliProjectOpen(event.payload, "event");
      },
    );
    unlistenCliProjectOpen = keepCleanup(cliProjectOpenCleanup);
    if (!unlistenCliProjectOpen) return;

    const pendingCliProjectOpen = await consumePendingCliProjectOpen();
    if (pendingCliProjectOpen && !disposed) {
      handleCliProjectOpen(pendingCliProjectOpen, "pending");
    }
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
  unlistenCliProjectOpen?.();
  unlistenPopupNavigate?.();
  unlistenInteraction = null;
  unlistenConversationActivity = null;
  unlistenMainNavigate = null;
  unlistenCliProjectOpen = null;
  unlistenPopupNavigate = null;
});
</script>

<template>
  <RouterView />
  <ContextMenuHost />
</template>
