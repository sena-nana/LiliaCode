<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { RouterLink, type RouteLocationRaw } from "vue-router";
import { AlertTriangle, Sparkles } from "lucide-vue-next";
import type { CodexAccountQuotaStatus, CodexAccountQuotaWindow } from "@lilia/contracts";
import { useConnectionStatus } from "../composables/useConnectionStatus";
import { getCodexAccountQuotaStatus } from "../services/chat";
import {
  clampPercent,
  formatDateTime,
  formatPercent,
  formatUnixSeconds,
  quotaPercentTone,
  quotaWindowLabel,
} from "../utils/quotaDisplay";

const QUOTA_STALE_MS = 60_000;

const props = withDefaults(defineProps<{
  to?: RouteLocationRaw | null;
  popoverId?: string;
}>(), {
  to: null,
  popoverId: "provider-conn-quota-popover",
});

const {
  activeBackend,
  statusFor,
  routerFor,
  nodeAvailable,
  codexCliAvailable,
  codexAppServer,
  refresh,
} = useConnectionStatus({ probe: false, loadBackend: false });

const activeStatus = computed(() => statusFor(activeBackend.value));
const officialQuota = ref<CodexAccountQuotaStatus | null>(null);
const quotaLoading = ref(false);
const quotaTooltipOpen = ref(false);
let quotaRequestSeq = 0;
let quotaInflight: Promise<void> | null = null;

const backendLabel = computed(() =>
  activeBackend.value === "codex" ? "Codex" : "Claude",
);

const badgeTag = computed(() => props.to ? RouterLink : "button");
const badgeAttrs = computed(() => props.to ? { to: props.to } : { type: "button" });

function codexRuntimeIssueText(): string {
  const status = codexAppServer.value;
  const issue = status?.issues.join(" ") || "Codex app-server 环境不满足。";
  if (status?.failureKind === "providerIncompatible") {
    return `${issue} 请确认当前 API 来源支持 OpenAI Responses API 与 Codex 模型白名单。点击进入设置。`;
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
    return `${backendLabel.value} API 未配置。点击进入设置。`;
  }
  if (s.connectionMode === "codex-account") {
    return "Codex · 官方账号";
  }
  return `${backendLabel.value} · ${s.effectiveUrl ?? "—"}`;
});

const isCodexOfficialAccount = computed(() =>
  activeBackend.value === "codex" &&
  connectionTone.value === "ok" &&
  routerFor("codex") === "codex-account" &&
  activeStatus.value?.connectionMode === "codex-account",
);

const quotaRows = computed(() => [
  { key: "fiveHour", label: "5 小时额度", badge: "5h", window: officialQuota.value?.fiveHour },
  { key: "weekly", label: "周额度", badge: "周", window: officialQuota.value?.weekly },
] as const);

const shouldShowQuotaRings = computed(() =>
  isCodexOfficialAccount.value && officialQuota.value?.available === true,
);

const quotaStatusLine = computed(() => {
  if (quotaLoading.value && !officialQuota.value) return "正在读取 Codex 官方额度…";
  if (officialQuota.value?.error) return officialQuota.value.error;
  if (officialQuota.value?.available) return "Codex 官方账号额度";
  return "Codex 官方额度暂无可用数据";
});

function quotaUnavailableStatus(error: unknown): CodexAccountQuotaStatus {
  return {
    available: false,
    connectionMode: "codex-account",
    limitId: null,
    limitName: null,
    planType: null,
    rateLimitReachedType: null,
    fiveHour: null,
    weekly: null,
    fetchedAt: Date.now(),
    error: String(error),
  };
}

function quotaRingStyle(window: CodexAccountQuotaWindow | null | undefined) {
  return {
    "--quota-progress": `${clampPercent(window?.usedPercent ?? 0)}%`,
  };
}

function quotaRingTone(window: CodexAccountQuotaWindow | null | undefined) {
  return window ? `sb-quota-ring--${quotaPercentTone(window.usedPercent)}` : "sb-quota-ring--empty";
}

function quotaUsageLine(window: CodexAccountQuotaWindow | null | undefined): string {
  if (!window) return "暂无数据";
  return `已用 ${formatPercent(window.usedPercent)} · 剩余 ${formatPercent(100 - window.usedPercent)}`;
}

function quotaFetchedLabel(status: CodexAccountQuotaStatus | null): string {
  if (!status?.fetchedAt) return "--";
  return formatDateTime(status.fetchedAt);
}

async function loadOfficialQuota() {
  if (!isCodexOfficialAccount.value) {
    clearOfficialQuota();
    return;
  }
  if (officialQuota.value?.fetchedAt && Date.now() - officialQuota.value.fetchedAt < QUOTA_STALE_MS) {
    return;
  }
  if (quotaInflight) return quotaInflight;

  const seq = ++quotaRequestSeq;
  quotaLoading.value = true;
  quotaInflight = (async () => {
    try {
      const result = await getCodexAccountQuotaStatus();
      if (seq === quotaRequestSeq) officialQuota.value = result;
    } catch (err) {
      if (seq === quotaRequestSeq) officialQuota.value = quotaUnavailableStatus(err);
    } finally {
      if (seq === quotaRequestSeq) quotaLoading.value = false;
      quotaInflight = null;
    }
  })();
  return quotaInflight;
}

function openQuotaDetails() {
  if (!isCodexOfficialAccount.value) return;
  quotaTooltipOpen.value = true;
  void loadOfficialQuota();
}

function closeQuotaDetails() {
  quotaTooltipOpen.value = false;
}

function clearOfficialQuota() {
  quotaRequestSeq += 1;
  officialQuota.value = null;
  quotaLoading.value = false;
  quotaTooltipOpen.value = false;
}

watch(
  isCodexOfficialAccount,
  (enabled) => {
    if (enabled) {
      void loadOfficialQuota();
      return;
    }
    clearOfficialQuota();
  },
  { immediate: true },
);

onMounted(() => {
  window.setTimeout(() => {
    void refresh(false).then(() => loadOfficialQuota());
  }, 0);
});
</script>

<template>
  <component
    :is="badgeTag"
    v-bind="badgeAttrs"
    class="sb-conn"
    :class="[
      `sb-conn--${connectionTone}`,
      { 'sb-conn--quota': shouldShowQuotaRings },
    ]"
    :title="connectionTooltip"
    :aria-label="connectionTooltip"
    :aria-describedby="quotaTooltipOpen ? popoverId : undefined"
    @mouseenter="openQuotaDetails"
    @mouseleave="closeQuotaDetails"
    @focus="openQuotaDetails"
    @blur="closeQuotaDetails"
  >
    <template v-if="connectionTone !== 'ok'">
      <AlertTriangle :size="12" aria-hidden="true" />
      <span class="sb-conn__label">{{ connectionTone === "error" ? "异常" : "未连接" }}</span>
    </template>
    <template v-else-if="activeStatus">
      <Sparkles :size="12" aria-hidden="true" />
      <span class="sb-conn__label">{{ backendLabel }}</span>
      <span v-if="shouldShowQuotaRings" class="sb-quota-rings" aria-hidden="true">
        <span
          v-for="row in quotaRows"
          :key="row.key"
          class="sb-quota-ring"
          :class="quotaRingTone(row.window)"
          :style="quotaRingStyle(row.window)"
        >
          <span>{{ row.badge }}</span>
        </span>
      </span>
    </template>
    <template v-else>
      <span class="sb-conn__label sb-conn__label--probing">检测中...</span>
    </template>

    <span
      v-if="quotaTooltipOpen"
      :id="popoverId"
      role="tooltip"
      class="sb-conn-popover"
    >
      <span class="sb-conn-popover__head">
        <strong>Codex 官方账号</strong>
        <small>{{ quotaStatusLine }}</small>
      </span>
      <span class="sb-conn-popover__meta">
        <span>{{ officialQuota?.planType || "套餐未知" }}</span>
        <span>{{ officialQuota?.limitName || officialQuota?.limitId || "Codex limit" }}</span>
        <span>{{ quotaLoading ? "读取中" : `查询 ${quotaFetchedLabel(officialQuota)}` }}</span>
      </span>
      <span class="sb-conn-popover__quota-list">
        <span v-for="row in quotaRows" :key="row.key" class="sb-conn-popover__quota-row">
          <span class="sb-conn-popover__quota-main">
            <strong>{{ row.label }}</strong>
            <small>{{ quotaWindowLabel(row.window) }}</small>
          </span>
          <span class="sb-conn-popover__quota-side">
            <span>{{ quotaUsageLine(row.window) }}</span>
            <small>重置 {{ formatUnixSeconds(row.window?.resetsAt) }}</small>
          </span>
        </span>
      </span>
      <span v-if="officialQuota?.rateLimitReachedType" class="sb-conn-popover__warn">
        已触发 {{ officialQuota.rateLimitReachedType }}
      </span>
      <span v-if="officialQuota?.error" class="sb-conn-popover__error">
        {{ officialQuota.error }}
      </span>
    </span>
  </component>
</template>
