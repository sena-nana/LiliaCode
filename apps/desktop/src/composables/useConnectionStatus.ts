/**
 * 跨页面共享的连接状态 — 让侧栏徽章和设置页面看的是同一份 EnvStatus。
 *
 * 用模块级 ref 而不是 Pinia/provide，因为整个状态就一份、读多写少，引入 Pinia
 * 反而是过度工程。所有调用 useConnectionStatus() 的组件都拿到同一份引用，
 * Settings 页面里点「重新检测」更新 ref 后，侧栏徽章会自动重渲。
 */

import { ref } from "vue";
import { checkEnv, type EnvStatus } from "../services/chat";

const status = ref<EnvStatus | null>(null);
const probing = ref(false);
let inflight: Promise<void> | null = null;

async function probeOnce() {
  if (inflight) return inflight;
  probing.value = true;
  inflight = (async () => {
    try { status.value = await checkEnv(); }
    catch (err) { console.error("[connection] checkEnv failed", err); }
    finally {
      probing.value = false;
      inflight = null;
    }
  })();
  return inflight;
}

export function useConnectionStatus() {
  // 没拿过状态就懒触一次；后续调用方拿到的是同一个 ref。
  if (status.value === null && !inflight) {
    void probeOnce();
  }
  return { status, probing, refresh: probeOnce };
}
