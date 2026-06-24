<script setup lang="ts">
import { defineAsyncComponent, onBeforeUnmount, onMounted, ref } from "vue";
import {
  beginPerfStage,
  installPerfObservers,
  measurePerfAsync,
  scheduleAfterPaint,
} from "../utils/perf";
import { createLazyLoadState } from "../utils/lazyLoadState";

const automationWorkspacePageLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "automations.workspace.load",
    async () => (await import("./automations/AutomationWorkspacePage.vue")).default,
  )
);

const AutomationWorkspacePage = defineAsyncComponent({
  suspensible: false,
  loader: () => automationWorkspacePageLoad.load(),
});

const workspaceReady = ref(false);
let deferredMountSeq = 0;
let cancelWorkspacePaint: (() => void) | null = null;
let cancelRoutePaint: (() => void) | null = null;

function cancelWorkspaceMountSchedule() {
  cancelWorkspacePaint?.();
  cancelWorkspacePaint = null;
  cancelRoutePaint?.();
  cancelRoutePaint = null;
}

function scheduleWorkspaceMount() {
  cancelWorkspaceMountSchedule();
  const seq = ++deferredMountSeq;
  const stage = beginPerfStage("automations.route.mount");
  cancelRoutePaint = scheduleAfterPaint(() => {
    cancelRoutePaint = null;
    stage.end("paint");
  });
  cancelWorkspacePaint = scheduleAfterPaint(() => {
    cancelWorkspacePaint = null;
    if (seq !== deferredMountSeq) return;
    workspaceReady.value = true;
  });
}

onMounted(() => {
  installPerfObservers();
  scheduleWorkspaceMount();
});

onBeforeUnmount(() => {
  deferredMountSeq += 1;
  cancelWorkspaceMountSchedule();
});
</script>

<template>
  <AutomationWorkspacePage v-if="workspaceReady" data-agent-id="automations.workspace" />
  <section v-else class="automations-shell" aria-busy="true" aria-label="自动化工作区加载中" data-agent-id="automations.loading">
    <div class="automations-shell__pulse" />
    <div class="automations-shell__body">
      <p class="automations-shell__eyebrow">Automations</p>
      <h1>正在准备自动化工作区</h1>
      <p>先让路由完成切换，再异步挂载画布和运行历史。</p>
    </div>
  </section>
</template>

<style scoped>
.automations-shell {
  display: grid;
  min-height: 100%;
  place-items: center;
  gap: 20px;
  padding: 40px;
  background:
    radial-gradient(circle at top, color-mix(in srgb, var(--accent-500) 14%, transparent), transparent 42%),
    linear-gradient(180deg, color-mix(in srgb, var(--surface-2) 90%, transparent), var(--surface-1));
}

.automations-shell__pulse {
  width: min(420px, 100%);
  height: 14px;
  border-radius: 999px;
  background:
    linear-gradient(
      90deg,
      color-mix(in srgb, var(--accent-500) 8%, var(--surface-3)),
      color-mix(in srgb, var(--accent-500) 34%, var(--surface-2)),
      color-mix(in srgb, var(--accent-500) 8%, var(--surface-3))
    );
  background-size: 220% 100%;
  animation: automations-shell-pulse 1.1s linear infinite;
}

.automations-shell__body {
  width: min(420px, 100%);
  padding: 20px 22px;
  border: 1px solid var(--border-subtle);
  border-radius: 18px;
  background: color-mix(in srgb, var(--surface-2) 92%, transparent);
  box-shadow: 0 14px 32px rgb(15 23 42 / 10%);
}

.automations-shell__eyebrow {
  margin: 0 0 8px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--text-tertiary);
}

.automations-shell__body h1 {
  margin: 0;
  font-size: 24px;
  line-height: 1.2;
  color: var(--text-primary);
}

.automations-shell__body p:last-child {
  margin: 10px 0 0;
  line-height: 1.6;
  color: var(--text-secondary);
}

@keyframes automations-shell-pulse {
  from {
    background-position: 100% 0;
  }

  to {
    background-position: 0 0;
  }
}
</style>
