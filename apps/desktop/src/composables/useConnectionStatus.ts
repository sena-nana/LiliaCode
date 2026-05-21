/**
 * 跨页面共享的连接状态——侧栏徽章和设置页看的是同一份 EnvStatusReport。
 * 用模块级 ref 而不是 Pinia，因为状态就一份、读多写少。
 */

import { computed, ref } from "vue";
import { checkEnv, type EnvStatusReport } from "../services/chat";
import type { BackendEnvStatus, ChatBackendKind, RouterMode } from "@lilia/contracts";

const report = ref<EnvStatusReport | null>(null);
const probing = ref(false);
let inflight: Promise<void> | null = null;

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

export function useConnectionStatus() {
  if (report.value === null && !inflight) {
    void probeOnce();
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
    probing,
    refresh: probeOnce,
    nodeAvailable,
    codexCliAvailable,
    ccSwitch,
    statusFor,
    routerFor,
  };
}
