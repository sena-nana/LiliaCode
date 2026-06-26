<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { RouterLink, type RouteLocationRaw } from "vue-router";
import { AlertTriangle, Download, Loader2, RefreshCw, Sparkles } from "lucide-vue-next";
import {
  chatBackendLabel,
  codexAccountQuotaWindowRemainingLine as quotaRemainingLine,
  codexAccountQuotaWindowRemainingPercent,
  codexRuntimeIssue,
  connectionModeIsUnconfigured,
  connectionModeUsesCodexAccount,
  type CodexAccountQuotaStatus,
  type CodexAccountQuotaWindow,
  routerModeUsesCodexAccount,
} from "@lilia/contracts";
import { useAnchoredOverlay } from "../composables/useAnchoredOverlay";
import { useConnectionStatus } from "../composables/useConnectionStatus";
import { getCodexAccountQuotaStatus } from "../services/chat";
import { cancelIdleRun, runWhenIdle, scheduleAfterPaint } from "../utils/perf";
import {
  codexQuotaUnavailableStatus,
  formatCompactNumber,
  formatUnixSeconds,
  quotaPercentTone,
} from "../utils/quotaDisplay";

const QUOTA_STALE_MS = 60_000;
const STARTUP_CONNECTION_REFRESH_DELAY_MS = 1_200;
const STARTUP_REMOTE_REFRESH_DELAY_MS = 2_500;

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
  checkCodexAppServerUpdate,
  installCodexAppServerUpdate,
  codexAppServerUpdateChecking,
  codexAppServerUpdating,
  codexAppServerUpdateError,
} = useConnectionStatus({ probe: false, loadBackend: false });

const activeStatus = computed(() => statusFor(activeBackend.value));
const badgeAnchorEl = ref<HTMLElement | null>(null);
const officialQuota = ref<CodexAccountQuotaStatus | null>(null);
const quotaLoading = ref(false);
const quotaTooltipOpen = ref(false);
const updateAnchorEl = ref<HTMLElement | null>(null);
const updateTooltipOpen = ref(false);
const closeTimerId = ref<number | null>(null);
const updateCloseTimerId = ref<number | null>(null);
let quotaRequestSeq = 0;
let quotaInflight: Promise<void> | null = null;
let startupSeq = 0;
let cancelStartupConnectionRefresh: (() => void) | null = null;
let cancelStartupRemoteRefresh: (() => void) | null = null;
let disposed = false;
let quotaOpenIntent = false;
const openState = computed(() => quotaTooltipOpen.value);
const updateOpenState = computed(() => updateTooltipOpen.value);
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
const {
  overlayEl: updatePopoverEl,
  overlayStyle: updatePopoverStyle,
  resolvedPlacement: resolvedUpdatePlacement,
  updatePosition: updatePopoverPosition,
} = useAnchoredOverlay({
  open: updateOpenState,
  anchorEl: updateAnchorEl,
  preferredPlacement,
  offset: 8,
});
void quotaPopoverEl;
void updatePopoverEl;

const backendLabel = computed(() => chatBackendLabel(activeBackend.value));

const badgeTag = computed(() => props.to ? RouterLink : "button");
const badgeAttrs = computed(() => props.to ? { to: props.to } : { type: "button" });

function codexRuntimeIssueText(): string {
  return `${codexRuntimeIssue(codexAppServer.value)} 点击进入设置。`;
}

const runtimeIssue = computed(() => {
  if (!nodeAvailable.value) return "未找到 node（v18+）。点击进入设置。";
  if (activeBackend.value === "codex" && !codexCliAvailable.value) {
    return "未找到 Lilia 内置 Codex app-server。点击进入设置。";
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
  () => connectionModeIsUnconfigured(activeStatus.value?.connectionMode) ||
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
  if (connectionModeIsUnconfigured(s.connectionMode)) {
    return `${backendLabel.value} API 未配置。点击进入设置。`;
  }
  if (connectionModeUsesCodexAccount(s.connectionMode)) {
    return "Codex · 官方账号";
  }
  return `${backendLabel.value} · ${s.effectiveUrl ?? "—"}`;
});

const isCodexOfficialAccount = computed(() => {
  const codexRouterMode = routerFor("codex");
  return activeBackend.value === "codex" &&
    connectionTone.value === "ok" &&
    codexRouterMode !== null &&
    routerModeUsesCodexAccount(codexRouterMode) &&
    connectionModeUsesCodexAccount(activeStatus.value?.connectionMode);
});

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
const resetCreditAvailableCount = computed(() =>
  officialQuota.value?.rateLimitResetCredits?.availableCount ?? 0,
);
const accountUsageSummary = computed(() => officialQuota.value?.accountUsage?.summary ?? null);
const accountUsageLine = computed(() => {
  const summary = accountUsageSummary.value;
  if (!summary) return null;
  const parts = [];
  if (summary.lifetimeTokens !== null) parts.push(`累计 ${formatCompactNumber(summary.lifetimeTokens)} tokens`);
  if (summary.currentStreakDays !== null) parts.push(`连续 ${summary.currentStreakDays} 天`);
  return parts.join(" · ") || null;
});

const shouldShowQuotaRings = computed(() =>
  isCodexOfficialAccount.value && Boolean(officialQuota.value?.fiveHour || officialQuota.value?.weekly),
);
const shouldShowCodexUpdate = computed(() =>
  activeBackend.value === "codex" &&
  connectionModeUsesCodexAccount(activeStatus.value?.connectionMode) &&
  (
    Boolean(codexAppServer.value?.updateAvailable) ||
    codexAppServer.value?.updateState === "downloading" ||
    codexAppServerUpdating.value
  ),
);
const codexUpdateState = computed(() => codexAppServer.value?.updateState ?? "idle");
const codexUpdateDownloading = computed(() => codexUpdateState.value === "downloading");
const codexUpdateReady = computed(() => codexUpdateState.value === "ready");
const codexUpdateProgressPercent = computed(() => codexAppServer.value?.updateProgressPercent ?? null);
const codexUpdateProgressStyle = computed(() => ({
  "--quota-progress": String(codexUpdateProgressPercent.value ?? 0),
}));
const codexUpdateTarget = computed(() =>
  codexAppServer.value?.preparedVersion ??
  codexAppServer.value?.latestVersion ??
  "最新版本",
);
const codexUpdateTitle = computed(() => {
  const current = codexAppServer.value?.version ?? "未安装";
  if (codexAppServerUpdating.value || codexUpdateState.value === "switching") {
    return `切换 Codex app-server：${current} -> ${codexUpdateTarget.value}`;
  }
  if (codexUpdateDownloading.value || codexAppServerUpdateChecking.value) {
    if (codexUpdateDownloading.value && codexUpdateProgressPercent.value !== null) {
      return `下载 Codex app-server（${codexUpdateProgressPercent.value}%）：${current} -> ${codexUpdateTarget.value}`;
    }
    return `下载 Codex app-server：${current} -> ${codexUpdateTarget.value}`;
  }
  if (codexUpdateReady.value) {
    return `切换 Codex app-server：${current} -> ${codexUpdateTarget.value}`;
  }
  return `准备 Codex app-server：${current} -> ${codexUpdateTarget.value}`;
});
const codexReleaseNotes = computed(() => codexAppServer.value?.releaseNotes ?? []);

function quotaRingStyle(window: CodexAccountQuotaWindow | null | undefined) {
  const remainingPercent = codexAccountQuotaWindowRemainingPercent(window);
  return {
    "--quota-progress": String(remainingPercent),
  };
}

function quotaRingTone(window: CodexAccountQuotaWindow | null | undefined) {
  return window ? `sb-quota-ring--${quotaPercentTone(window.usedPercent)}` : "sb-quota-ring--empty";
}

function cancelCloseTimer() {
  if (closeTimerId.value !== null) {
    window.clearTimeout(closeTimerId.value);
    closeTimerId.value = null;
  }
}

function cancelUpdateCloseTimer() {
  if (updateCloseTimerId.value !== null) {
    window.clearTimeout(updateCloseTimerId.value);
    updateCloseTimerId.value = null;
  }
}

function scheduleCloseQuotaDetails() {
  cancelCloseTimer();
  closeTimerId.value = window.setTimeout(() => {
    quotaTooltipOpen.value = false;
    closeTimerId.value = null;
  }, 80);
}

function scheduleDelayedIdle(delayMs: number, run: () => void): () => void {
  let active = true;
  let idleHandle: number | null = null;
  let timerId: number | null = window.setTimeout(() => {
    timerId = null;
    if (!active) return;
    idleHandle = runWhenIdle(() => {
      idleHandle = null;
      if (active) run();
    });
  }, delayMs);
  return () => {
    active = false;
    if (timerId !== null) {
      window.clearTimeout(timerId);
      timerId = null;
    }
    if (idleHandle !== null) {
      cancelIdleRun(idleHandle);
      idleHandle = null;
    }
  };
}

function scheduleAfterPaintDelayedIdle(delayMs: number, run: () => void): () => void {
  let cancelDelay: (() => void) | null = null;
  let cancelPaint: (() => void) | null = scheduleAfterPaint(() => {
    cancelPaint = null;
    cancelDelay = scheduleDelayedIdle(delayMs, run);
  });
  return () => {
    cancelPaint?.();
    cancelPaint = null;
    cancelDelay?.();
    cancelDelay = null;
  };
}

function cancelStartupRefreshSchedule() {
  startupSeq += 1;
  cancelStartupConnectionRefresh?.();
  cancelStartupConnectionRefresh = null;
  cancelStartupRemoteRefresh?.();
  cancelStartupRemoteRefresh = null;
}

function shouldRunStartupRemoteRefresh(): boolean {
  return activeBackend.value === "codex" &&
    connectionModeUsesCodexAccount(activeStatus.value?.connectionMode);
}

function scheduleStartupRemoteRefresh(seq: number) {
  cancelStartupRemoteRefresh?.();
  cancelStartupRemoteRefresh = scheduleDelayedIdle(STARTUP_REMOTE_REFRESH_DELAY_MS, () => {
    if (disposed || seq !== startupSeq || !shouldRunStartupRemoteRefresh()) return;
    void Promise.all([
      checkCodexAppServerUpdate(),
      loadOfficialQuota(),
    ]).catch((err) => {
      console.error("[provider-connection] startup remote refresh failed", err);
    });
  });
}

function scheduleStartupRefresh() {
  cancelStartupRefreshSchedule();
  const seq = startupSeq;
  cancelStartupConnectionRefresh = scheduleAfterPaintDelayedIdle(
    STARTUP_CONNECTION_REFRESH_DELAY_MS,
    () => {
      if (disposed || seq !== startupSeq) return;
      void refresh(false).then(() => {
        if (disposed || seq !== startupSeq) return;
        scheduleStartupRemoteRefresh(seq);
      }).catch((err) => {
        console.error("[provider-connection] startup connection refresh failed", err);
      });
    },
  );
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
      if (!disposed && seq === quotaRequestSeq) officialQuota.value = result;
    } catch (err) {
      if (!disposed && seq === quotaRequestSeq) officialQuota.value = codexQuotaUnavailableStatus(err);
    } finally {
      if (!disposed && seq === quotaRequestSeq) quotaLoading.value = false;
      quotaInflight = null;
    }
  })();
  return quotaInflight;
}

async function openQuotaDetails() {
  if (!isCodexOfficialAccount.value) return;
  quotaOpenIntent = true;
  cancelCloseTimer();
  await loadOfficialQuota();
  if (disposed || !quotaOpenIntent || !isCodexOfficialAccount.value) return;
  quotaTooltipOpen.value = true;
  void nextTick(() => {
    if (!disposed && quotaTooltipOpen.value) void updateQuotaPopoverPosition();
  });
}

function closeQuotaDetails() {
  quotaOpenIntent = false;
  scheduleCloseQuotaDetails();
}

function scheduleCloseUpdateDetails() {
  cancelUpdateCloseTimer();
  updateCloseTimerId.value = window.setTimeout(() => {
    updateTooltipOpen.value = false;
    updateCloseTimerId.value = null;
  }, 80);
}

function openUpdateDetails() {
  if (!shouldShowCodexUpdate.value && !codexAppServerUpdating.value) return;
  cancelUpdateCloseTimer();
  updateTooltipOpen.value = true;
  void nextTick(() => {
    if (!disposed && updateTooltipOpen.value) void updatePopoverPosition();
  });
}

function closeUpdateDetails() {
  scheduleCloseUpdateDetails();
}

async function installUpdate() {
  if (disposed) return;
  await installCodexAppServerUpdate();
}

function clearOfficialQuota() {
  cancelCloseTimer();
  quotaOpenIntent = false;
  quotaRequestSeq += 1;
  officialQuota.value = null;
  quotaLoading.value = false;
  quotaTooltipOpen.value = false;
}

watch(
  isCodexOfficialAccount,
  (enabled) => {
    if (enabled) return;
    cancelStartupRemoteRefresh?.();
    cancelStartupRemoteRefresh = null;
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

watch(
  () => [
    updateTooltipOpen.value,
    codexAppServer.value?.latestVersion ?? "",
    codexAppServer.value?.releaseNotes.join("\n") ?? "",
    codexAppServerUpdating.value,
    codexAppServerUpdateError.value ?? "",
  ] as const,
  ([open]) => {
    if (!open) return;
    void updatePopoverPosition();
  },
);

onMounted(() => {
  disposed = false;
  scheduleStartupRefresh();
});

onBeforeUnmount(() => {
  disposed = true;
  quotaOpenIntent = false;
  quotaRequestSeq += 1;
  quotaInflight = null;
  quotaLoading.value = false;
  cancelStartupRefreshSchedule();
  cancelCloseTimer();
  cancelUpdateCloseTimer();
});
</script>

<template>
  <span
    ref="badgeAnchorEl"
    class="sb-conn-anchor"
  >
    <component
      :is="badgeTag"
      v-bind="badgeAttrs"
      class="sb-conn"
      :class="[
        `sb-conn--${connectionTone}`,
        { 'sb-conn--quota': shouldShowQuotaRings },
      ]"
      data-agent-id="provider-connection.badge"
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
    <button
      v-if="shouldShowCodexUpdate || codexAppServerUpdating"
      ref="updateAnchorEl"
      type="button"
      class="sb-conn-update"
      data-agent-id="provider-connection.codex-update"
      :disabled="codexAppServerUpdating || codexUpdateDownloading || !codexUpdateReady"
      :title="codexUpdateTitle"
      :aria-label="codexUpdateTitle"
      :aria-describedby="updateTooltipOpen ? `${popoverId}-update` : undefined"
      @mouseenter="openUpdateDetails"
      @mouseleave="closeUpdateDetails"
      @focus="openUpdateDetails"
      @blur="closeUpdateDetails"
      @click.stop.prevent="installUpdate"
    >
      <Loader2
        v-if="codexAppServerUpdating || codexAppServerUpdateChecking"
        :size="12"
        class="is-spinning"
        aria-hidden="true"
      />
      <span
        v-else-if="codexUpdateDownloading"
        class="sb-quota-ring"
        :class="{ 'sb-quota-ring--empty': codexUpdateProgressPercent === null }"
        :style="codexUpdateProgressStyle"
        aria-hidden="true"
      >
        <svg viewBox="0 0 16 16" focusable="false" aria-hidden="true">
          <circle class="sb-quota-ring__track" cx="8" cy="8" r="6" pathLength="100" />
          <circle class="sb-quota-ring__value" cx="8" cy="8" r="6" pathLength="100" />
        </svg>
      </span>
      <RefreshCw v-else-if="codexUpdateReady" :size="12" aria-hidden="true" />
      <Download v-else :size="12" aria-hidden="true" />
    </button>
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
      <span v-if="resetCreditAvailableCount > 0" class="sb-conn-popover__warn">
        重置次数可用 {{ resetCreditAvailableCount }} 次
      </span>
      <span v-if="accountUsageLine" class="sb-conn-popover__warn">
        {{ accountUsageLine }}
      </span>
      <span v-if="officialQuota?.usageError" class="sb-conn-popover__warn">
        {{ officialQuota.usageError }}
      </span>
      <span v-if="officialQuota?.error" class="sb-conn-popover__error">
        {{ officialQuota.error }}
      </span>
    </span>
  </Teleport>
  <Teleport to="body">
    <span
      v-if="updateTooltipOpen"
      :id="`${popoverId}-update`"
      ref="updatePopoverEl"
      role="tooltip"
      class="sb-conn-popover sb-conn-popover--update"
      :class="`sb-conn-popover--${resolvedUpdatePlacement}`"
      :data-placement="resolvedUpdatePlacement"
      :style="updatePopoverStyle"
      @mouseenter="openUpdateDetails"
      @mouseleave="closeUpdateDetails"
    >
      <span class="sb-conn-popover__update-title">{{ codexUpdateTitle }}</span>
      <span v-if="codexReleaseNotes.length" class="sb-conn-popover__update-list">
        <span v-for="note in codexReleaseNotes" :key="note">{{ note }}</span>
      </span>
      <span v-else class="sb-conn-popover__warn">暂未获取更新内容。</span>
      <span v-if="codexAppServerUpdateError" class="sb-conn-popover__error">
        {{ codexAppServerUpdateError }}
      </span>
    </span>
  </Teleport>
</template>
