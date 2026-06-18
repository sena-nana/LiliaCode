<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { RouterLink, type RouteLocationRaw } from "vue-router";
import { AlertTriangle, Sparkles } from "lucide-vue-next";
import type { CodexAccountQuotaStatus, CodexAccountQuotaWindow } from "@lilia/contracts";
import { useAnchoredOverlay } from "../composables/useAnchoredOverlay";
import { useConnectionStatus } from "../composables/useConnectionStatus";
import { getCodexAccountQuotaStatus } from "../services/chat";
import {
  clampPercent,
  formatPercent,
  formatUnixSeconds,
  quotaPercentTone,
} from "../utils/quotaDisplay";

const QUOTA_STALE_MS = 60_000;

const props = withDefaults(defineProps<{
  to?: RouteLocationRaw | null;
  popoverId?: string;
  preferredPlacement?: "top-start" | "top-end" | "bottom-start" | "bottom-end";
}>(), {
  to: null,
  popoverId: "provider-conn-quota-popover",
  preferredPlacement: "top-start",
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
const badgeAnchorEl = ref<HTMLElement | null>(null);
const officialQuota = ref<CodexAccountQuotaStatus | null>(null);
const quotaLoading = ref(false);
const quotaTooltipOpen = ref(false);
const closeTimerId = ref<number | null>(null);
let quotaRequestSeq = 0;
let quotaInflight: Promise<void> | null = null;
const openState = computed(() => quotaTooltipOpen.value);
const preferredPlacement = computed(() => props.preferredPlacement);
const {
  overlayEl: quotaPopoverEl,
  overlayStyle: quotaPopoverStyle,
  resolvedPlacement: resolvedQuotaPlacement,
  updatePosition: updateQuotaPopoverPosition,
} = useAnchoredOverlay({
  open: openState,
  anchorEl: badgeAnchorEl,
  preferredPlacement,
  offset: 8,
});

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
  { key: "fiveHour", window: officialQuota.value?.fiveHour, suffix: "" },
  { key: "weekly", window: officialQuota.value?.weekly, suffix: "" },
] as const);
const sparkQuotaRows = computed(() => [
  { key: "sparkFiveHour", window: officialQuota.value?.sparkFiveHour, suffix: "Spark" },
  { key: "sparkWeekly", window: officialQuota.value?.sparkWeekly, suffix: "Spark" },
] as const);
const quotaDetailRows = computed(() => [
  ...quotaRows.value,
  ...sparkQuotaRows.value.filter((row) => row.window),
]);

const shouldShowQuotaRings = computed(() =>
  isCodexOfficialAccount.value && Boolean(officialQuota.value?.fiveHour || officialQuota.value?.weekly),
);

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
    sparkFiveHour: null,
    sparkWeekly: null,
    fetchedAt: Date.now(),
    error: String(error),
  };
}

function quotaRingStyle(window: CodexAccountQuotaWindow | null | undefined) {
  const remainingPercent = quotaRemainingPercent(window);
  return {
    "--quota-progress": String(remainingPercent),
  };
}

function quotaRingTone(window: CodexAccountQuotaWindow | null | undefined) {
  return window ? `sb-quota-ring--${quotaPercentTone(window.usedPercent)}` : "sb-quota-ring--empty";
}

function quotaRemainingPercent(window: CodexAccountQuotaWindow | null | undefined): number {
  return clampPercent(100 - (window?.usedPercent ?? 0));
}

function quotaWindowShortLabel(window: CodexAccountQuotaWindow | null | undefined): string {
  const mins = window?.windowDurationMins;
  if (!mins || mins <= 0) return "额度";
  if (mins % 1440 === 0) return `${mins / 1440}d`;
  if (mins % 60 === 0) return `${mins / 60}h`;
  return `${mins}m`;
}

function quotaRemainingLine(window: CodexAccountQuotaWindow | null | undefined, suffix = ""): string {
  if (!window) return "额度 · 暂无数据";
  const base = `${quotaWindowShortLabel(window)} · 剩余 ${formatPercent(quotaRemainingPercent(window))}`;
  return suffix ? `${base} · ${suffix}` : base;
}

function cancelCloseTimer() {
  if (closeTimerId.value !== null) {
    window.clearTimeout(closeTimerId.value);
    closeTimerId.value = null;
  }
}

function scheduleCloseQuotaDetails() {
  cancelCloseTimer();
  closeTimerId.value = window.setTimeout(() => {
    quotaTooltipOpen.value = false;
    closeTimerId.value = null;
  }, 80);
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
  cancelCloseTimer();
  quotaTooltipOpen.value = true;
  void loadOfficialQuota();
  void nextTick(() => updateQuotaPopoverPosition());
}

function closeQuotaDetails() {
  scheduleCloseQuotaDetails();
}

function clearOfficialQuota() {
  cancelCloseTimer();
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

watch(
  () => [
    quotaTooltipOpen.value,
    officialQuota.value?.available ?? false,
    officialQuota.value?.error ?? "",
    quotaDetailRows.value.length,
    quotaLoading.value,
  ] as const,
  ([open]) => {
    if (!open) return;
    void updateQuotaPopoverPosition();
  },
);

onMounted(() => {
  window.setTimeout(() => {
    void refresh(false).then(() => loadOfficialQuota());
  }, 0);
});

onBeforeUnmount(() => {
  cancelCloseTimer();
});
</script>

<template>
  <span
    ref="badgeAnchorEl"
    class="sb-conn-anchor"
    @mouseenter="openQuotaDetails"
    @mouseleave="closeQuotaDetails"
  >
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
            <svg viewBox="0 0 16 16" focusable="false" aria-hidden="true">
              <circle class="sb-quota-ring__track" cx="8" cy="8" r="6" pathLength="100" />
              <circle class="sb-quota-ring__value" cx="8" cy="8" r="6" pathLength="100" />
            </svg>
          </span>
        </span>
      </template>
      <template v-else>
        <span class="sb-conn__label sb-conn__label--probing">检测中...</span>
      </template>
    </component>
  </span>
  <Teleport to="body">
    <span
      v-if="quotaTooltipOpen"
      :id="popoverId"
      ref="quotaPopoverEl"
      role="tooltip"
      class="sb-conn-popover"
      :class="`sb-conn-popover--${resolvedQuotaPlacement}`"
      :data-placement="resolvedQuotaPlacement"
      :style="quotaPopoverStyle"
      @mouseenter="openQuotaDetails"
      @mouseleave="closeQuotaDetails"
    >
      <span v-if="officialQuota?.available" class="sb-conn-popover__quota-list">
        <span v-for="row in quotaDetailRows" :key="row.key" class="sb-conn-popover__quota-row">
          <span
            class="sb-conn-popover__quota-meter"
            :class="quotaRingTone(row.window)"
            :style="quotaRingStyle(row.window)"
            aria-hidden="true"
          >
            <span />
          </span>
          <span class="sb-conn-popover__quota-foot">
            <span>{{ quotaRemainingLine(row.window, row.suffix) }}</span>
            <span>刷新 {{ formatUnixSeconds(row.window?.resetsAt) }}</span>
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
  </Teleport>
</template>
