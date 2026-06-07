<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, ref } from "vue";
import { RouterLink } from "vue-router";
import {
  AlertTriangle,
  Import,
  Puzzle,
  Settings,
  Sparkles,
} from "lucide-vue-next";
import { useConnectionStatus } from "../../composables/useConnectionStatus";

const ConversationImportDialog = defineAsyncComponent(
  () => import("./ConversationImportDialog.vue"),
);

const { activeBackend, statusFor, nodeAvailable, codexCliAvailable, codexAppServer, refresh } =
  useConnectionStatus({ probe: false, loadBackend: false });

const importOpen = ref(false);
const activeStatus = computed(() => statusFor(activeBackend.value));

const backendLabel = computed(() =>
  activeBackend.value === "codex" ? "Codex" : "Claude",
);

function codexRuntimeIssueText(): string {
  const status = codexAppServer.value;
  const issue = status?.issues.join(" ") || "Codex app-server 环境不满足。";
  if (status?.failureKind === "providerIncompatible") {
    return `${issue} 请确认上游 provider 支持 OpenAI Responses API 与 Codex 模型白名单。点击进入设置。`;
  }
  return `${issue} 点击进入设置。`;
}

const runtimeIssue = computed(() => {
  if (!nodeAvailable.value) return "未找到 node（v18+）。点击进入设置。";
  if (activeBackend.value === "codex" && !codexCliAvailable.value) {
    return "未找到 codex CLI。点击进入设置。";
  }
  if (
    activeBackend.value === "codex" &&
    codexAppServer.value &&
    !codexAppServer.value.supportsRequiredProtocol
  ) {
    return codexRuntimeIssueText();
  }
  return null;
});

const hasConnectionIssue = computed(
  () => activeStatus.value?.connectionMode === "unconfigured" ||
    activeStatus.value === null,
);

const connectionTone = computed(() => {
  if (runtimeIssue.value) return "error";
  if (hasConnectionIssue.value) return "warn";
  return "ok";
});

const connectionTooltip = computed(() => {
  if (runtimeIssue.value) return runtimeIssue.value;
  const s = activeStatus.value;
  if (!s) return "正在检测 agent 连接…";
  if (s.connectionMode === "unconfigured") {
    return `${backendLabel.value} 未连接。CC-Switch 代理不可达。点击进入设置。`;
  }
  return `${backendLabel.value} · ${s.effectiveUrl ?? "—"}`;
});

onMounted(() => {
  window.setTimeout(() => {
    void refresh(false);
  }, 0);
});
</script>

<template>
  <div class="sb-footer">
    <RouterLink to="/settings" class="sb-footer__btn" active-class="is-active" title="设置" aria-label="设置">
      <Settings :size="14" aria-hidden="true" />
    </RouterLink>

    <RouterLink to="/plugins" class="sb-footer__btn" active-class="is-active" title="插件 / 技能" aria-label="插件 / 技能">
      <Puzzle :size="14" aria-hidden="true" />
    </RouterLink>

    <button
      type="button"
      class="sb-footer__btn"
      title="从 Claude / Codex 导入对话"
      aria-label="从 Claude / Codex 导入对话"
      @click="importOpen = true"
    >
      <Import :size="14" aria-hidden="true" />
    </button>

    <RouterLink to="/settings" class="sb-conn" :class="`sb-conn--${connectionTone}`"
      :title="connectionTooltip" :aria-label="connectionTooltip">
      <template v-if="connectionTone !== 'ok'">
        <AlertTriangle :size="12" aria-hidden="true" />
        <span class="sb-conn__label">{{ connectionTone === "error" ? "异常" : "未连接" }}</span>
      </template>
      <template v-else-if="activeStatus">
        <Sparkles :size="12" aria-hidden="true" />
        <span class="sb-conn__label">{{ backendLabel }}</span>
      </template>
      <template v-else>
        <span class="sb-conn__label sb-conn__label--probing">检测中…</span>
      </template>
    </RouterLink>
  </div>

  <ConversationImportDialog
    v-if="importOpen"
    @close="importOpen = false"
  />
</template>
