/**
 * 跨页面共享连接状态和当前 Agent provider。
 * 用模块级 ref 而不是 Pinia，因为状态就一份、读多写少。
 */

import { computed, ref } from "vue";
import {
  checkEnv,
  getActiveBackend,
  setActiveBackend as persistActiveBackend,
  type EnvStatusReport,
} from "../services/chat";
import type { BackendEnvStatus, ChatBackendKind, RouterMode } from "@lilia/contracts";

const report = ref<EnvStatusReport | null>(null);
const activeBackend = ref<ChatBackendKind>("claude");
const probing = ref(false);
let inflight: Promise<void> | null = null;
let backendInflight: Promise<ChatBackendKind> | null = null;
let activeBackendLoaded = false;

async function probeOnce() {
  if (inflight) return inflight;
  probing.value = true;
  inflight = (async () => {
    try { report.value = await checkEnv(); }
    catch (err) { console.error("[connection] checkEnv failed", err); }
    finally {
      probing.value = false;
      inflight = null;
    }
  })();
  return inflight;
}

async function loadActiveBackend(): Promise<ChatBackendKind> {
  if (backendInflight) return backendInflight;
  backendInflight = getActiveBackend()
    .then((backend) => {
      activeBackend.value = backend === "codex" ? "codex" : "claude";
      activeBackendLoaded = true;
      return activeBackend.value;
    })
    .catch((err) => {
      console.error("[connection] getActiveBackend failed", err);
      activeBackend.value = "claude";
      activeBackendLoaded = true;
      return activeBackend.value;
    })
    .finally(() => {
      backendInflight = null;
    });
  return backendInflight;
}

async function setActiveBackend(backend: ChatBackendKind): Promise<ChatBackendKind> {
  const next = backend === "codex" ? "codex" : "claude";
  const previous = activeBackend.value;
  activeBackend.value = next;
  try {
    await persistActiveBackend(next);
    activeBackendLoaded = true;
    return activeBackend.value;
  } catch (err) {
    activeBackend.value = previous;
    throw err;
  }
}

export function useConnectionStatus() {
  if (report.value === null && !inflight) {
    void probeOnce();
  }
  if (!activeBackendLoaded && !backendInflight) {
    void loadActiveBackend();
  }

  const nodeAvailable = computed(() => report.value?.nodeAvailable ?? false);
  const codexCliAvailable = computed(() => report.value?.codexCliAvailable ?? false);
  const ccSwitch = computed(() => report.value?.ccSwitch ?? null);

  function statusFor(backend: ChatBackendKind): BackendEnvStatus | null {
    return report.value?.backends?.[backend] ?? null;
  }
  function routerFor(backend: ChatBackendKind): RouterMode | null {
    return report.value?.routerModes?.[backend] ?? null;
  }

  return {
    report,
    activeBackend,
    probing,
    refresh: probeOnce,
    setActiveBackend,
    nodeAvailable,
    codexCliAvailable,
    ccSwitch,
    statusFor,
    routerFor,
  };
}
