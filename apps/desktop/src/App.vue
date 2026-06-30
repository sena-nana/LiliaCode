<script setup lang="ts">
import { defineAsyncComponent, onBeforeUnmount, onMounted } from "vue";
import { RouterView, useRouter } from "vue-router";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  CLI_PROJECT_OPEN_EVENT_NAME,
  MAIN_NAVIGATE_EVENT_NAME,
  POPUP_NAVIGATE_EVENT_NAME,
  type AppNavigateEvent,
} from "@lilia/contracts";
import {
  consumePendingCliProjectOpen,
  openCliProject,
  type CliProjectOpenPayload,
} from "./services/cliProjectOpen";
import {
  createLazyLoadState,
  installCombinedUnlisten,
  measurePerfAsync,
  scheduleAfterPaint,
} from "@lilia/ui";

const contextMenuHostLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "app.context-menu-host.load",
    async () => (await import("@lilia/ui/components/ContextMenuHost")).default,
    { detail: "ContextMenuHost" },
  )
);
const appUpdateHostLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "app.update-host.load",
    async () => (await import("./components/AppUpdateHost.vue")).default,
    { detail: "AppUpdateHost" },
  )
);
const deferredBridgeLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "app.bridge.import",
    () => Promise.all([
      import("./composables/useAgentInteractionBridge"),
      import("./composables/useConversationActivity"),
    ]),
  )
);

const ContextMenuHost = defineAsyncComponent({
  loader: () => contextMenuHostLoad.load(),
  suspensible: false,
});
const AppUpdateHost = defineAsyncComponent({
  loader: () => appUpdateHostLoad.load(),
  suspensible: false,
});

let unlistenDeferredBridges: (() => void) | null = null;
let unlistenWindowNavigation: (() => void) | null = null;
let cancelDeferredBridgePaint: (() => void) | null = null;
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
    await deferredBridgeLoad.load();
  if (disposed) return;

  unlistenDeferredBridges = keepCleanup(await installCombinedUnlisten([
    () => measurePerfAsync(
      "app.bridge.agent.install",
      () => installAgentInteractionBridge(),
      { detail: appWindow.label },
    ),
    () => measurePerfAsync(
      "app.bridge.activity.install",
      () => installConversationActivityBridge(),
      { detail: appWindow.label },
    ),
  ]));
}

async function installWindowNavigationListeners() {
  if (isMainWindow) {
    const unlisten = await installCombinedUnlisten([
      () => listen<AppNavigateEvent>(MAIN_NAVIGATE_EVENT_NAME, (event) => {
        const route = event.payload.route;
        if (typeof route === "string" && route.startsWith("/")) {
          void router.push(route);
        }
      }),
      () => listen<CliProjectOpenPayload>(
        CLI_PROJECT_OPEN_EVENT_NAME,
        (event) => {
          handleCliProjectOpen(event.payload, "event");
        },
      ),
    ]);
    unlistenWindowNavigation = keepCleanup(unlisten);
    if (!unlistenWindowNavigation) return;

    const pendingCliProjectOpen = await consumePendingCliProjectOpen();
    if (pendingCliProjectOpen && !disposed) {
      handleCliProjectOpen(pendingCliProjectOpen, "pending");
    }
    return;
  }

  if (isPopupWindow) {
    unlistenWindowNavigation = keepCleanup(await listen<AppNavigateEvent>(
      POPUP_NAVIGATE_EVENT_NAME,
      (event) => {
        const route = event.payload.route;
        if (typeof route === "string" && route.startsWith("/popup/")) {
          void router.replace(route);
        }
      },
    ));
  }
}

onMounted(async () => {
  cancelDeferredBridgePaint = scheduleAfterPaint(() => {
    cancelDeferredBridgePaint = null;
    if (disposed) return;
    void installDeferredBridges().catch((err) => {
      console.error("[app] install deferred bridges failed", err);
    });
  });
  await installWindowNavigationListeners().catch((err) => {
    console.error("[app] install window navigation listeners failed", err);
  });
});

onBeforeUnmount(() => {
  disposed = true;
  cancelDeferredBridgePaint?.();
  cancelDeferredBridgePaint = null;
  unlistenDeferredBridges?.();
  unlistenWindowNavigation?.();
  unlistenDeferredBridges = null;
  unlistenWindowNavigation = null;
});
</script>

<template>
  <RouterView />
  <ContextMenuHost />
  <AppUpdateHost />
</template>

