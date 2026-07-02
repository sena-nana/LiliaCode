import { computed, shallowRef, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { check, type DownloadEvent } from "@tauri-apps/plugin-updater";
import { APP_RESTART_COMMAND } from "@lilia/contracts/appCommandsContract.mjs";

type AppUpdate = NonNullable<Awaited<ReturnType<typeof check>>>;

export type DesktopAppUpdatePhase =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "installing"
  | "restarting"
  | "failed";

interface DesktopAppUpdaterOptions {
  autoCheck?: boolean;
  isProduction?: boolean;
  windowLabel?: string;
  checkUpdate?: typeof check;
  restart?: () => Promise<void>;
}

const promptedVersions = new Set<string>();
let startupCheckInflight: Promise<void> | null = null;

export function resetDesktopAppUpdaterForTest() {
  promptedVersions.clear();
  startupCheckInflight = null;
}

function shouldInstallInThisWindow(options: DesktopAppUpdaterOptions) {
  const windowLabel = options.windowLabel ?? getCurrentWindow().label;
  const isProduction = options.isProduction ?? import.meta.env.PROD;
  return windowLabel === "main" && isProduction;
}

export function useDesktopAppUpdater(options: DesktopAppUpdaterOptions = {}) {
  const phase = ref<DesktopAppUpdatePhase>("idle");
  const update = shallowRef<AppUpdate | null>(null);
  const progressPercent = ref<number | null>(null);
  const downloadedBytes = ref(0);
  const contentLength = ref<number | null>(null);
  const checkUpdate = options.checkUpdate ?? check;
  const restart = options.restart ?? (() => invoke<void>(APP_RESTART_COMMAND));

  const version = computed(() => update.value?.version ?? null);
  const currentVersion = computed(() => update.value?.currentVersion ?? null);
  const busy = computed(() => (
    phase.value === "checking" ||
    phase.value === "downloading" ||
    phase.value === "installing" ||
    phase.value === "restarting"
  ));
  const open = computed(() =>
    phase.value === "available" ||
    phase.value === "downloading" ||
    phase.value === "installing" ||
    phase.value === "restarting" ||
    phase.value === "failed"
  );
  const title = computed(() => {
    if (phase.value === "failed") return "更新失败";
    if (phase.value === "downloading") return "正在下载更新";
    if (phase.value === "installing") return "正在安装更新";
    if (phase.value === "restarting") return "正在重启 LiliaCode";
    return version.value ? `发现 LiliaCode ${version.value}` : "发现 LiliaCode 更新";
  });
  const message = computed(() => {
    if (phase.value === "failed") {
      return "更新没有完成。请稍后重试，或从 GitHub Release 下载新版安装包。";
    }
    if (phase.value === "downloading") {
      return progressPercent.value === null
        ? "正在下载新版安装包。"
        : `正在下载新版安装包（${progressPercent.value}%）。`;
    }
    if (phase.value === "installing") return "正在安装新版，完成后将重启应用。";
    if (phase.value === "restarting") return "更新已安装，正在重启应用。";
    if (version.value && currentVersion.value) {
      return `当前版本 ${currentVersion.value}，可更新到 ${version.value}。安装完成后会重启应用。`;
    }
    return "新版已准备好。安装完成后会重启应用。";
  });
  const confirmText = computed(() => phase.value === "failed" ? "重试" : "更新并重启");
  const busyText = computed(() => {
    if (phase.value === "downloading" && progressPercent.value !== null) {
      return `下载中 ${progressPercent.value}%`;
    }
    if (phase.value === "installing") return "安装中...";
    if (phase.value === "restarting") return "重启中...";
    return "处理中...";
  });

  async function checkForUpdate() {
    if (!shouldInstallInThisWindow(options) || phase.value === "checking") return;
    phase.value = "checking";
    try {
      const next = await checkUpdate();
      if (!next) {
        phase.value = "idle";
        return;
      }
      if (promptedVersions.has(next.version)) {
        phase.value = "idle";
        return;
      }
      update.value = next;
      promptedVersions.add(next.version);
      phase.value = "available";
    } catch {
      phase.value = "failed";
    }
  }

  function handleDownloadEvent(event: DownloadEvent) {
    if (event.event === "Started") {
      downloadedBytes.value = 0;
      contentLength.value = typeof event.data.contentLength === "number"
        ? event.data.contentLength
        : null;
      progressPercent.value = contentLength.value ? 0 : null;
      return;
    }
    if (event.event === "Progress") {
      downloadedBytes.value += event.data.chunkLength;
      progressPercent.value = contentLength.value
        ? Math.min(99, Math.floor((downloadedBytes.value / contentLength.value) * 100))
        : null;
      return;
    }
    progressPercent.value = 100;
    phase.value = "installing";
  }

  async function installUpdate() {
    if (!update.value || busy.value) return;
    progressPercent.value = null;
    downloadedBytes.value = 0;
    contentLength.value = null;
    phase.value = "downloading";
    try {
      await update.value.downloadAndInstall(handleDownloadEvent);
      phase.value = "restarting";
      await restart();
    } catch {
      phase.value = "failed";
    }
  }

  async function confirmUpdate() {
    if (phase.value === "failed" && !update.value) {
      await checkForUpdate();
      return;
    }
    await installUpdate();
  }

  function dismissUpdate() {
    if (busy.value) return;
    phase.value = "idle";
  }

  function startAutoCheck() {
    if (options.autoCheck === false || !shouldInstallInThisWindow(options)) return;
    if (!startupCheckInflight) {
      startupCheckInflight = checkForUpdate().finally(() => {
        startupCheckInflight = null;
      });
    }
  }

  return {
    phase,
    update,
    version,
    currentVersion,
    progressPercent,
    busy,
    open,
    title,
    message,
    confirmText,
    busyText,
    checkForUpdate,
    installUpdate,
    confirmUpdate,
    dismissUpdate,
    startAutoCheck,
  };
}
