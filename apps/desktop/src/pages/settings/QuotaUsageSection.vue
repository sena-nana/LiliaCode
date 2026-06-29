<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import type { ChartData, ChartOptions, ChartType, TooltipItem } from "chart.js";
import {
  AlertTriangle,
  Coins,
  Database,
  RotateCcw,
  RefreshCw,
} from "@lucide/vue";
import {
  type CodexAccountQuotaStatus,
  connectionModeUsesCodexAccount,
  codexAccountQuotaCreditsLabel,
  codexRateLimitResetCreditConsumeOutcomeLabel,
  DEFAULT_QUOTA_USAGE_STATS_DAYS,
  QUOTA_USAGE_STATS_BACKEND_FILTERS,
  QUOTA_USAGE_STATS_DAYS,
  type QuotaUsageStats,
  type QuotaUsageStatsBackendFilter,
  type QuotaUsageStatsDays,
  quotaUsageStatsBackendFilterLabel,
  quotaUsageStatsDaysLabel,
} from "@lilia/contracts";
import QuotaChartCanvas from "./QuotaChartCanvas.vue";
import {
  consumeCodexRateLimitResetCredit,
  getCodexAccountQuotaStatus,
  getQuotaUsageStats,
} from "../../services/chat";
import {
  codexQuotaUnavailableStatus,
  formatCompactNumber,
  formatDateTime,
  formatPercent,
  formatUnixSeconds,
  quotaWindowLabel,
} from "../../utils/quotaDisplay";

type BackendOption = { value: QuotaUsageStatsBackendFilter; label: string };
type DaysOption = { value: QuotaUsageStatsDays; label: string };
type QuotaBreakdownItem = {
  key: string;
  label: string;
  value: number;
  meta: string;
  color: string;
};
type NumericChartData = ChartData<ChartType, number[], string>;

const backendOptions: BackendOption[] = [
  ...QUOTA_USAGE_STATS_BACKEND_FILTERS.map((value) => ({
    value,
    label: quotaUsageStatsBackendFilterLabel(value),
  })),
];
const daysOptions: DaysOption[] = [
  ...QUOTA_USAGE_STATS_DAYS.map((value) => ({ value, label: quotaUsageStatsDaysLabel(value) })),
];
const maxStatsDays = Math.max(...QUOTA_USAGE_STATS_DAYS);
const chartPalette = {
  input: "var(--accent)",
  output: "var(--ok)",
  cacheRead: "var(--warn)",
  cacheCreate: "var(--text-muted)",
  total: "var(--text)",
  grid: "var(--border-soft)",
  muted: "var(--text-muted)",
};
const breakdownPalette = [
  "var(--accent)",
  "var(--ok)",
  "var(--warn)",
  "var(--text)",
  "var(--text-muted)",
];

const selectedBackend = ref<QuotaUsageStatsBackendFilter>("all");
const selectedDays = ref<QuotaUsageStatsDays>(DEFAULT_QUOTA_USAGE_STATS_DAYS);
const stats = ref<QuotaUsageStats | null>(null);
const officialQuota = ref<CodexAccountQuotaStatus | null>(null);
const loading = ref(false);
const quotaLoading = ref(false);
const resetCreditLoading = ref(false);
const resetCreditMessage = ref("");
const resetCreditError = ref("");
const error = ref("");
let requestSeq = 0;
let quotaRequestSeq = 0;
let disposed = false;

const totalRecords = computed(() => stats.value?.cost.totalRecordCount ?? 0);
const hasUsage = computed(() => totalRecords.value > 0);
const totalTokens = computed(() => stats.value?.totals.totalTokens ?? 0);
const refreshing = computed(() => loading.value || quotaLoading.value);
const showOfficialQuota = computed(() =>
  connectionModeUsesCodexAccount(officialQuota.value?.connectionMode),
);
const officialQuotaWindows = computed(() => [
  { key: "fiveHour", window: officialQuota.value?.fiveHour },
  { key: "weekly", window: officialQuota.value?.weekly },
]);
const sparkQuotaWindows = computed(() => [
  { key: "sparkFiveHour", window: officialQuota.value?.sparkFiveHour },
  { key: "sparkWeekly", window: officialQuota.value?.sparkWeekly },
]);
const officialQuotaGroups = computed(() => [
  { key: "codex", label: "", rows: officialQuotaWindows.value },
  { key: "spark", label: "Spark额度", rows: sparkQuotaWindows.value.filter((row) => row.window) },
].filter((group) => group.rows.some((row) => row.window)));
const officialQuotaCreditRows = computed(() => [
  {
    key: "codex",
    label: "Workspace credit",
    credits: officialQuota.value?.credits ?? null,
  },
  ...(officialQuota.value?.sparkCredits
    ? [{
      key: "spark",
      label: "Spark workspace credit",
      credits: officialQuota.value.sparkCredits,
    }]
    : []),
]);
const hasOfficialQuotaWindow = computed(() => officialQuotaGroups.value.length > 0);
const resetCreditAvailableCount = computed(() =>
  officialQuota.value?.rateLimitResetCredits?.availableCount ?? 0,
);
const canConsumeResetCredit = computed(() =>
  showOfficialQuota.value &&
  Boolean(officialQuota.value?.available) &&
  resetCreditAvailableCount.value > 0 &&
  !quotaLoading.value &&
  !resetCreditLoading.value,
);
const accountUsageSummaryRows = computed(() => {
  const summary = officialQuota.value?.accountUsage?.summary;
  if (!summary) return [];
  return [
    { key: "lifetime", label: "累计 Token", value: summary.lifetimeTokens, suffix: "tokens" },
    { key: "peak", label: "单日峰值", value: summary.peakDailyTokens, suffix: "tokens" },
    { key: "streak", label: "连续活跃", value: summary.currentStreakDays, suffix: "天" },
    { key: "longest", label: "最长连续", value: summary.longestStreakDays, suffix: "天" },
  ].filter((row) => row.value !== null);
});
const accountUsageBuckets = computed(() =>
  (officialQuota.value?.accountUsage?.dailyUsageBuckets ?? []).slice(-14),
);
const sortedBackends = computed(() =>
  [...(stats.value?.backends ?? [])].sort((a, b) => b.totalTokens - a.totalTokens),
);
const projectBreakdown = computed(() =>
  topBreakdown(
    stats.value?.projects ?? [],
    (row) => row.projectId ?? "__unassigned__",
    (row) => row.projectName,
    (row) => row.totalTokens,
    (row) => `${formatCompactNumber(row.totalTokens)} tokens`,
  ),
);
const conversationBreakdown = computed(() =>
  topBreakdown(
    stats.value?.conversations ?? [],
    (row) => row.taskId,
    (row) => row.taskTitle,
    (row) => row.totalTokens,
    (row) => `${formatCompactNumber(row.totalTokens)} tokens`,
  ),
);
const toolBreakdown = computed(() =>
  topBreakdown(
    stats.value?.tools ?? [],
    (row) => row.key,
    (row) => row.label,
    (row) => row.callCount,
    (row) => `${formatNumber(row.callCount)} 次`,
  ),
);
const usageTrendChartLabel = computed(
  () => `Token 用量趋势（${quotaUsageStatsBackendFilterLabel(selectedBackend.value)} · 近 ${selectedDays.value} 天）`,
);
const usageTrendChartData = computed<NumericChartData>(() => {
  const daily = stats.value?.daily ?? [];
  return {
    labels: daily.map((bucket) => formatDay(bucket.dayStart)),
    datasets: [
      tokenBarDataset("输入", daily.map((bucket) => bucket.inputTokens), chartPalette.input),
      tokenBarDataset("输出", daily.map((bucket) => bucket.outputTokens), chartPalette.output),
      tokenBarDataset("缓存命中", daily.map((bucket) => bucket.cacheReadTokens), chartPalette.cacheRead),
      tokenBarDataset("缓存写入", daily.map((bucket) => bucket.cacheCreationTokens), chartPalette.cacheCreate),
      {
        type: "line",
        label: "总量",
        data: daily.map((bucket) => bucket.totalTokens),
        yAxisID: "total",
        borderColor: chartPalette.total,
        backgroundColor: chartPalette.total,
        borderWidth: 2,
        pointRadius: 2,
        pointHoverRadius: 3,
        tension: 0.25,
        fill: false,
        order: 0,
      },
    ],
  };
});
const usageTrendChartOptions = computed<ChartOptions>(() => ({
  interaction: {
    intersect: false,
    mode: "index",
  },
  plugins: {
    legend: {
      position: "bottom",
      labels: {
        boxWidth: 10,
        boxHeight: 10,
        color: chartPalette.muted,
        usePointStyle: true,
      },
    },
    tooltip: {
      callbacks: {
        label(context: TooltipItem<ChartType>) {
          const value = typeof context.parsed.y === "number" ? context.parsed.y : context.parsed;
          return `${context.dataset.label ?? ""}: ${formatCompactNumber(Number(value))} tokens`;
        },
        footer(items: TooltipItem<ChartType>[]) {
          const index = items[0]?.dataIndex;
          const bucket = typeof index === "number" ? stats.value?.daily[index] : null;
          if (!bucket) return "";
          return [
            `成本 ${formatRecordCost(bucket.knownCostUsd)}`,
            `记录 ${formatNumber(bucket.recordCount)}`,
          ];
        },
      },
    },
  },
  scales: {
    x: {
      stacked: true,
      grid: {
        display: false,
      },
      ticks: {
        autoSkip: selectedDays.value === maxStatsDays,
        maxRotation: 0,
        color: chartPalette.muted,
      },
    },
    y: {
      stacked: true,
      beginAtZero: true,
      grid: {
        color: chartPalette.grid,
      },
      ticks: {
        color: chartPalette.muted,
        callback(value) {
          return formatCompactNumber(Number(value));
        },
      },
    },
    total: {
      display: false,
      beginAtZero: true,
      grid: {
        display: false,
      },
    },
  },
}));
const backendChartData = computed<NumericChartData>(() => ({
  labels: sortedBackends.value.map((row) => quotaUsageStatsBackendFilterLabel(row.backend)),
  datasets: [
    {
      label: "Token",
      data: sortedBackends.value.map((row) => row.totalTokens),
      backgroundColor: sortedBackends.value.map((_, index) =>
        breakdownPalette[index % breakdownPalette.length]
      ),
      borderWidth: 0,
      borderRadius: 4,
    },
  ],
}));
const backendChartOptions = computed<ChartOptions>(() => ({
  indexAxis: "y",
  plugins: {
    legend: {
      display: false,
    },
    tooltip: {
      callbacks: {
        label(context: TooltipItem<ChartType>) {
          const value = Number(context.raw ?? 0);
          return `${formatCompactNumber(value)} tokens · ${backendShareText(value)}`;
        },
      },
    },
  },
  scales: {
    x: {
      beginAtZero: true,
      grid: {
        color: chartPalette.grid,
      },
      ticks: {
        color: chartPalette.muted,
        callback(value) {
          return formatCompactNumber(Number(value));
        },
      },
    },
    y: {
      grid: {
        display: false,
      },
      ticks: {
        color: chartPalette.muted,
      },
    },
  },
}));
const projectBreakdownChartData = computed(() => breakdownChartData(projectBreakdown.value));
const conversationBreakdownChartData = computed(() => breakdownChartData(conversationBreakdown.value));
const toolBreakdownChartData = computed(() => breakdownChartData(toolBreakdown.value));
const breakdownChartOptions = computed<ChartOptions>(() => ({
  cutout: "62%",
  plugins: {
    legend: {
      display: false,
    },
    tooltip: {
      callbacks: {
        label(context: TooltipItem<ChartType>) {
          const value = Number(context.raw ?? 0);
          return `${context.label}: ${formatCompactNumber(value)} · ${breakdownShareText(context)}`;
        },
      },
    },
  },
}));
const accountUsageChartData = computed<NumericChartData>(() => ({
  labels: accountUsageBuckets.value.map((bucket) => bucket.startDate.slice(5)),
  datasets: [
    {
      label: "Token",
      data: accountUsageBuckets.value.map((bucket) => bucket.tokens),
      backgroundColor: chartPalette.input,
      borderColor: chartPalette.input,
      borderWidth: 0,
      borderRadius: 3,
    },
  ],
}));
const accountUsageChartOptions = computed<ChartOptions>(() => ({
  plugins: {
    legend: {
      display: false,
    },
    tooltip: {
      callbacks: {
        label(context: TooltipItem<ChartType>) {
          return `${formatNumber(Number(context.raw ?? 0))} tokens`;
        },
      },
    },
  },
  scales: {
    x: {
      display: false,
      grid: {
        display: false,
      },
    },
    y: {
      display: false,
      beginAtZero: true,
      grid: {
        display: false,
      },
    },
  },
}));
const costText = computed(() => {
  const coverage = stats.value?.cost;
  if (!coverage || coverage.totalRecordCount === 0 || coverage.knownCostUsd === null) return "--";
  const value = formatCost(coverage.knownCostUsd);
  return coverage.costRecordCount < coverage.totalRecordCount ? `${value} 已知` : value;
});
const costCoverageText = computed(() => {
  const coverage = stats.value?.cost;
  if (!coverage || coverage.totalRecordCount === 0) return "无新增记录";
  if (coverage.costRecordCount === 0) return "未上报成本";
  if (coverage.costRecordCount === coverage.totalRecordCount) return "成本记录完整";
  return `${coverage.costRecordCount}/${coverage.totalRecordCount} 条记录含成本`;
});

function formatNumber(value: number) {
  return new Intl.NumberFormat("zh-CN").format(value);
}

function formatCost(value: number) {
  return `$${value.toFixed(value >= 1 ? 2 : 4)}`;
}

function formatRecordCost(value: number | null) {
  return value === null ? "--" : formatCost(value);
}

function topBreakdown<T>(
  rows: T[],
  keyOf: (row: T) => string,
  labelOf: (row: T) => string,
  valueOf: (row: T) => number,
  metaOf: (row: T) => string,
): QuotaBreakdownItem[] {
  const normalized = rows
    .map((row) => ({
      key: keyOf(row),
      label: labelOf(row),
      value: Math.max(0, valueOf(row)),
      meta: metaOf(row),
    }))
    .filter((row) => row.value > 0)
    .sort((a, b) => b.value - a.value)
    .slice(0, 5);
  return normalized.map((row, index) => ({
    ...row,
    color: breakdownPalette[index % breakdownPalette.length],
  }));
}

function formatDay(value: number) {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
  }).format(new Date(value));
}

function tokenBarDataset(label: string, data: number[], color: string) {
  return {
    type: "bar" as const,
    label,
    data,
    backgroundColor: color,
    borderColor: color,
    borderWidth: 0,
    borderRadius: 3,
    stack: "tokens",
  };
}

function backendShareText(tokens: number) {
  if (totalTokens.value <= 0) return "0%";
  return `${Math.round((tokens / totalTokens.value) * 100)}%`;
}

function breakdownChartData(items: QuotaBreakdownItem[]): NumericChartData {
  return {
    labels: items.map((item) => item.label),
    datasets: [
      {
        data: items.map((item) => item.value),
        backgroundColor: items.map((item) => item.color),
        borderColor: "var(--bg)",
        borderWidth: 2,
        hoverOffset: 3,
      },
    ],
  };
}

function breakdownShareText(context: TooltipItem<ChartType>) {
  const values = context.dataset.data.map((value) => Number(value) || 0);
  const total = values.reduce((sum, value) => sum + value, 0);
  if (total <= 0) return "0%";
  return `${Math.round((Number(context.raw ?? 0) / total) * 100)}%`;
}

function resetCreditText(count: number) {
  return count > 0 ? `可用 ${formatNumber(count)} 次` : "暂无可用重置次数";
}

function accountUsageValue(value: number | null, suffix: string) {
  if (value === null) return "--";
  return suffix === "tokens" ? `${formatCompactNumber(value)} tokens` : `${formatNumber(value)} ${suffix}`;
}

function createIdempotencyKey() {
  return globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

async function loadStats() {
  const seq = ++requestSeq;
  loading.value = true;
  error.value = "";
  try {
    const result = await getQuotaUsageStats({
      days: selectedDays.value,
      backend: selectedBackend.value,
    });
    if (!disposed && seq === requestSeq) stats.value = result;
  } catch (err) {
    if (!disposed && seq === requestSeq) error.value = String(err);
  } finally {
    if (!disposed && seq === requestSeq) loading.value = false;
  }
}

async function loadOfficialQuota() {
  const seq = ++quotaRequestSeq;
  quotaLoading.value = true;
  try {
    const result = await getCodexAccountQuotaStatus();
    if (!disposed && seq === quotaRequestSeq) officialQuota.value = result;
  } catch (err) {
    if (!disposed && seq === quotaRequestSeq) {
      officialQuota.value = codexQuotaUnavailableStatus(err);
    }
  } finally {
    if (!disposed && seq === quotaRequestSeq) quotaLoading.value = false;
  }
}

async function consumeResetCredit() {
  if (disposed || !canConsumeResetCredit.value) return;
  resetCreditLoading.value = true;
  resetCreditMessage.value = "";
  resetCreditError.value = "";
  try {
    const result = await consumeCodexRateLimitResetCredit({
      idempotencyKey: createIdempotencyKey(),
    });
    if (disposed) return;
    officialQuota.value = result.status;
    resetCreditMessage.value = codexRateLimitResetCreditConsumeOutcomeLabel(result.outcome);
  } catch (err) {
    if (!disposed) resetCreditError.value = String(err);
  } finally {
    if (!disposed) resetCreditLoading.value = false;
  }
}

async function refreshAll() {
  if (disposed) return;
  resetCreditMessage.value = "";
  resetCreditError.value = "";
  await Promise.all([loadStats(), loadOfficialQuota()]);
}

async function selectBackend(backend: QuotaUsageStatsBackendFilter) {
  if (selectedBackend.value === backend) return;
  selectedBackend.value = backend;
  await loadStats();
}

async function selectDays(days: QuotaUsageStatsDays) {
  if (selectedDays.value === days) return;
  selectedDays.value = days;
  await loadStats();
}

onMounted(() => {
  disposed = false;
  void refreshAll();
});

onBeforeUnmount(() => {
  disposed = true;
  requestSeq += 1;
  quotaRequestSeq += 1;
});
</script>

<template>
  <div class="quota-toolbar" data-agent-id="settings.quota">
    <div class="ui-segmented" role="radiogroup" aria-label="额度统计后端">
      <button
        v-for="opt in backendOptions"
        :key="opt.value"
        type="button"
        role="radio"
        :data-agent-id="`settings.quota.backend.${opt.value}`"
        :aria-checked="selectedBackend === opt.value"
        :class="{ 'is-active': selectedBackend === opt.value }"
        :disabled="loading"
        @click="selectBackend(opt.value)"
      >
        {{ opt.label }}
      </button>
    </div>
    <div class="quota-toolbar__actions">
      <button
        type="button"
        class="ui-button ui-button--ghost quota-panel__refresh"
        data-agent-id="settings.quota.refresh"
        :disabled="refreshing"
        @click="refreshAll"
      >
        <RefreshCw :size="12" :class="{ 'is-spinning': refreshing }" aria-hidden="true" />
        刷新
      </button>
      <div class="ui-segmented" role="radiogroup" aria-label="额度统计时间范围">
        <button
          v-for="opt in daysOptions"
          :key="opt.value"
          type="button"
          role="radio"
          :data-agent-id="`settings.quota.days.${opt.value}`"
          :aria-checked="selectedDays === opt.value"
          :class="{ 'is-active': selectedDays === opt.value }"
          :disabled="loading"
          @click="selectDays(opt.value)"
        >
          {{ opt.label }}
        </button>
      </div>
    </div>
  </div>

  <div v-if="error" class="card quota-panel">
    <div class="conn-banner conn-banner--err">
      <AlertTriangle :size="16" aria-hidden="true" />
      <div>
        <div class="conn-banner__title">额度统计</div>
        <div class="conn-banner__hint">{{ error }}</div>
      </div>
    </div>
  </div>

  <div v-if="showOfficialQuota" class="card quota-panel quota-official">
    <div class="quota-official__head">
      <div>
        <h2>
          <span class="card-h2__title">Codex 官方额度</span>
        </h2>
        <span>
          {{ officialQuota?.planType ? `计划 ${officialQuota.planType}` : "官方账号" }}
          <template v-if="officialQuota?.limitName"> · {{ officialQuota.limitName }}</template>
        </span>
      </div>
      <span class="ui-badge ui-badge--muted">
        {{ quotaLoading ? "读取中" : `查询 ${formatDateTime(officialQuota?.fetchedAt ?? Date.now())}` }}
      </span>
    </div>
    <div class="quota-official__actions">
      <span class="ui-badge ui-badge--muted">
        重置次数 {{ resetCreditText(resetCreditAvailableCount) }}
      </span>
      <button
        type="button"
        class="ui-button ui-button--ghost quota-official__reset"
        data-agent-id="settings.quota.reset-credit.consume"
        :disabled="!canConsumeResetCredit"
        @click="consumeResetCredit"
      >
        <RotateCcw :size="12" :class="{ 'is-spinning': resetCreditLoading }" aria-hidden="true" />
        {{ resetCreditLoading ? "使用中" : "使用重置次数" }}
      </button>
    </div>
    <div
      v-for="group in officialQuotaGroups"
      :key="group.key"
      class="quota-official__section"
    >
      <strong v-if="group.label">{{ group.label }}</strong>
      <div class="quota-official__grid">
        <div
          v-for="row in group.rows"
          :key="row.key"
          class="quota-official-window"
        >
          <strong>{{ row.window ? formatPercent(row.window.usedPercent) : "--" }}</strong>
          <small>{{ quotaWindowLabel(row.window) }}</small>
          <small>重置 {{ formatUnixSeconds(row.window?.resetsAt) }}</small>
        </div>
      </div>
    </div>
    <div class="quota-official__credits" aria-label="Codex 官方重置次数">
      <div
        v-for="row in officialQuotaCreditRows"
        :key="row.key"
        class="quota-official-credit"
      >
        <span>{{ row.label }}</span>
        <strong>{{ codexAccountQuotaCreditsLabel(row.credits) }}</strong>
      </div>
    </div>
    <div v-if="!hasOfficialQuotaWindow" class="quota-official__empty">
      暂无官方额度数据
    </div>
    <div v-if="resetCreditMessage" class="quota-official__message">
      {{ resetCreditMessage }}
    </div>
    <div v-if="resetCreditError" class="quota-official__error">
      {{ resetCreditError }}
    </div>
    <div
      v-if="accountUsageSummaryRows.length || accountUsageBuckets.length || officialQuota?.usageError"
      class="quota-official-usage"
    >
      <div class="quota-official-usage__head">
        <strong>官方账号用量</strong>
        <span v-if="officialQuota?.usageError">{{ officialQuota.usageError }}</span>
      </div>
      <div v-if="accountUsageSummaryRows.length" class="quota-official-usage__metrics">
        <div
          v-for="row in accountUsageSummaryRows"
          :key="row.key"
          class="quota-official-usage__metric"
        >
          <span>{{ row.label }}</span>
          <strong>{{ accountUsageValue(row.value, row.suffix) }}</strong>
        </div>
      </div>
      <QuotaChartCanvas
        v-if="accountUsageBuckets.length"
        class="quota-official-usage__chart"
        type="bar"
        label="官方账号每日用量"
        :data="accountUsageChartData"
        :options="accountUsageChartOptions"
      />
    </div>
    <div v-if="officialQuota?.error" class="quota-official__error">
      {{ officialQuota.error }}
    </div>
  </div>

  <div class="card quota-panel">
    <h2>
      <span class="card-h2__title">统计摘要</span>
      <span class="ui-badge ui-badge--muted">{{ quotaUsageStatsBackendFilterLabel(selectedBackend) }} · 近 {{ selectedDays }} 天</span>
    </h2>

    <div class="quota-metrics" aria-label="额度统计摘要">
      <div class="quota-metric">
        <span>总 Token</span>
        <strong>{{ formatNumber(stats?.totals.totalTokens ?? 0) }}</strong>
      </div>
      <div class="quota-metric">
        <span>输入</span>
        <strong>{{ formatNumber(stats?.totals.inputTokens ?? 0) }}</strong>
      </div>
      <div class="quota-metric">
        <span>输出</span>
        <strong>{{ formatNumber(stats?.totals.outputTokens ?? 0) }}</strong>
      </div>
      <div class="quota-metric">
        <span>缓存命中</span>
        <strong>{{ formatNumber(stats?.totals.cacheReadTokens ?? 0) }}</strong>
      </div>
      <div class="quota-metric">
        <span>缓存写入</span>
        <strong>{{ formatNumber(stats?.totals.cacheCreationTokens ?? 0) }}</strong>
      </div>
      <div class="quota-metric quota-metric--cost">
        <span>已知成本</span>
        <strong>{{ costText }}</strong>
        <small>{{ costCoverageText }}</small>
      </div>
    </div>
  </div>

  <div class="card quota-panel">
    <h2>
      <span class="card-h2__title">
        <Database :size="14" aria-hidden="true" />
        趋势
      </span>
      <span class="ui-badge ui-badge--muted">{{ quotaUsageStatsBackendFilterLabel(selectedBackend) }} · 近 {{ selectedDays }} 天</span>
    </h2>

    <div v-if="!hasUsage && !loading" class="quota-empty">
      暂无新增额度数据
    </div>

    <div v-else class="quota-chart-wrap">
      <QuotaChartCanvas
        class="quota-chart"
        type="bar"
        :label="usageTrendChartLabel"
        :data="usageTrendChartData"
        :options="usageTrendChartOptions"
      />
    </div>
  </div>

  <div class="card quota-panel">
    <h2>
      <span class="card-h2__title">
        <Coins :size="14" aria-hidden="true" />
        后端分布
      </span>
    </h2>

    <div v-if="sortedBackends.length" class="quota-backends">
      <QuotaChartCanvas
        class="quota-backends__chart"
        type="bar"
        label="后端 Token 分布"
        :data="backendChartData"
        :options="backendChartOptions"
      />
      <div class="quota-backends__list">
        <div
          v-for="row in sortedBackends"
          :key="row.backend"
          class="quota-backend-row"
        >
          <span>{{ quotaUsageStatsBackendFilterLabel(row.backend) }}</span>
          <strong>{{ formatCompactNumber(row.totalTokens) }} tokens · {{ backendShareText(row.totalTokens) }}</strong>
        </div>
      </div>
    </div>

    <div v-else class="quota-empty quota-empty--compact">
      暂无后端分布
    </div>
  </div>

  <section class="card quota-panel quota-breakdown" aria-label="项目消耗">
    <h2>
      <span class="card-h2__title">项目消耗</span>
      <span class="ui-badge ui-badge--muted">按 Token</span>
    </h2>
    <div class="quota-breakdown__body">
      <QuotaChartCanvas
        v-if="projectBreakdown.length"
        class="quota-breakdown__chart"
        type="doughnut"
        label="项目消耗图表"
        :data="projectBreakdownChartData"
        :options="breakdownChartOptions"
      />
      <div v-else class="quota-breakdown__chart quota-breakdown__chart--empty" aria-hidden="true" />
      <div class="quota-breakdown__list">
        <div
          v-for="item in projectBreakdown"
          :key="item.key"
          class="quota-breakdown__row"
        >
          <i :style="{ background: item.color }" />
          <span>{{ item.label }}</span>
          <strong>{{ item.meta }}</strong>
        </div>
        <div v-if="projectBreakdown.length === 0" class="quota-breakdown__empty">
          暂无项目消耗
        </div>
      </div>
    </div>
  </section>

  <section class="card quota-panel quota-breakdown" aria-label="对话消耗">
    <h2>
      <span class="card-h2__title">对话消耗</span>
      <span class="ui-badge ui-badge--muted">按 Token</span>
    </h2>
    <div class="quota-breakdown__body">
      <QuotaChartCanvas
        v-if="conversationBreakdown.length"
        class="quota-breakdown__chart"
        type="doughnut"
        label="对话消耗图表"
        :data="conversationBreakdownChartData"
        :options="breakdownChartOptions"
      />
      <div v-else class="quota-breakdown__chart quota-breakdown__chart--empty" aria-hidden="true" />
      <div class="quota-breakdown__list">
        <div
          v-for="item in conversationBreakdown"
          :key="item.key"
          class="quota-breakdown__row"
        >
          <i :style="{ background: item.color }" />
          <span>{{ item.label }}</span>
          <strong>{{ item.meta }}</strong>
        </div>
        <div v-if="conversationBreakdown.length === 0" class="quota-breakdown__empty">
          暂无对话消耗
        </div>
      </div>
    </div>
  </section>

  <section class="card quota-panel quota-breakdown" aria-label="工具活跃度">
    <h2>
      <span class="card-h2__title">工具活跃度</span>
      <span class="ui-badge ui-badge--muted">按调用次数统计</span>
    </h2>
    <div class="quota-breakdown__body">
      <QuotaChartCanvas
        v-if="toolBreakdown.length"
        class="quota-breakdown__chart"
        type="doughnut"
        label="工具活跃度图表"
        :data="toolBreakdownChartData"
        :options="breakdownChartOptions"
      />
      <div v-else class="quota-breakdown__chart quota-breakdown__chart--empty" aria-hidden="true" />
      <div class="quota-breakdown__list">
        <div
          v-for="item in toolBreakdown"
          :key="item.key"
          class="quota-breakdown__row"
        >
          <i :style="{ background: item.color }" />
          <span>{{ item.label }}</span>
          <strong>{{ item.meta }}</strong>
        </div>
        <div v-if="toolBreakdown.length === 0" class="quota-breakdown__empty">
          暂无工具调用
        </div>
      </div>
    </div>
  </section>

  <div v-if="stats?.recent.length" class="card quota-panel">
    <h2>
      <span class="card-h2__title">最近记录</span>
    </h2>
    <div class="quota-recent">
      <div
        v-for="record in stats.recent"
        :key="record.eventId"
        class="quota-recent__row"
      >
        <div class="quota-recent__main">
          <strong>{{ quotaUsageStatsBackendFilterLabel(record.backend) }}</strong>
          <span>{{ formatDateTime(record.createdAt) }}</span>
        </div>
        <span>{{ formatCompactNumber(record.totalTokens) }} tokens</span>
        <span>{{ formatRecordCost(record.knownCostUsd) }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.quota-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-start;
  gap: 8px;
  margin-bottom: 12px;
}

.quota-toolbar__actions {
  min-width: 0;
  display: inline-flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  margin-left: auto;
}

.quota-panel {
  display: grid;
  gap: 12px;
}

.quota-panel h2 {
  min-width: 0;
  margin-bottom: 0;
}

.quota-panel h2 .ui-badge {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quota-panel__refresh {
  height: 28px;
  padding-inline: 8px;
  font-size: 12px;
}

.quota-metrics {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: 8px;
}

.quota-metric {
  min-width: 0;
  padding: 9px 10px;
  border: 1px solid var(--border-soft);
  border-radius: 7px;
  background: var(--bg);
}

.quota-metric span,
.quota-metric small {
  display: block;
  overflow: hidden;
  color: var(--text-muted);
  font-size: 11px;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quota-metric strong {
  display: block;
  margin-top: 3px;
  overflow: hidden;
  color: var(--text);
  font-size: 17px;
  font-variant-numeric: tabular-nums;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quota-metric--cost strong {
  color: var(--accent);
}

.quota-official {
  display: grid;
  gap: 9px;
}

.quota-official__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.quota-official__head > div {
  min-width: 0;
  display: grid;
  gap: 2px;
}

.quota-official__head > div > span,
.quota-official-window span,
.quota-official-window small,
.quota-official-credit span,
.quota-official__empty,
.quota-official__message,
.quota-official__error {
  overflow: hidden;
  color: var(--text-muted);
  font-size: 11px;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quota-official__actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.quota-official__reset {
  height: 28px;
  padding-inline: 8px;
  font-size: 12px;
}

.quota-official__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.quota-official__section {
  display: grid;
  gap: 7px;
}

.quota-official__section > strong {
  color: var(--text);
  font-size: 12px;
}

.quota-official-window,
.quota-official-credit {
  min-width: 0;
  display: grid;
  gap: 3px;
  border: 1px solid var(--border-soft);
  border-radius: 7px;
  background: var(--bg-subtle);
}

.quota-official-window {
  padding: 9px 10px;
}

.quota-official-window strong {
  color: var(--accent);
  font-size: 18px;
  font-variant-numeric: tabular-nums;
  line-height: 1.15;
}

.quota-official__credits {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.quota-official-credit {
  padding: 8px 10px;
}

.quota-official-credit strong {
  overflow: hidden;
  color: var(--text);
  font-size: 13px;
  line-height: 1.25;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quota-official__empty {
  padding: 8px 2px;
}

.quota-official__message {
  color: var(--ok);
}

.quota-official__error {
  color: var(--warn);
}

.quota-official-usage {
  display: grid;
  gap: 8px;
  padding-top: 8px;
  border-top: 1px solid var(--border-soft);
}

.quota-official-usage__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.quota-official-usage__head strong {
  color: var(--text);
  font-size: 12px;
}

.quota-official-usage__head span {
  overflow: hidden;
  color: var(--warn);
  font-size: 11px;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quota-official-usage__metrics {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
}

.quota-official-usage__metric {
  min-width: 0;
  display: grid;
  gap: 3px;
  padding: 8px 10px;
  border: 1px solid var(--border-soft);
  border-radius: 7px;
  background: var(--bg-subtle);
}

.quota-official-usage__metric span,
.quota-official-usage__metric strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quota-official-usage__metric span {
  color: var(--text-muted);
  font-size: 11px;
}

.quota-official-usage__metric strong {
  color: var(--text);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}

.quota-official-usage__chart {
  height: 52px;
}

.quota-chart-wrap {
  min-width: 0;
  display: grid;
  gap: 10px;
}

.quota-chart {
  height: 248px;
}

.quota-backends {
  display: grid;
  gap: 8px;
}

.quota-backends__chart {
  height: 116px;
}

.quota-backends__list {
  display: grid;
  gap: 6px;
}

.quota-breakdown {
  min-width: 0;
  display: grid;
  gap: 9px;
}

.quota-breakdown__empty {
  overflow: hidden;
  color: var(--text-muted);
  font-size: 11px;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quota-breakdown__body {
  display: grid;
  grid-template-columns: 66px minmax(0, 1fr);
  gap: 10px;
  align-items: center;
}

.quota-breakdown__chart {
  width: 66px;
  height: 66px;
}

.quota-breakdown__chart--empty {
  border: 1px solid var(--border-soft);
  border-radius: 50%;
  background: var(--bg-subtle);
}

.quota-breakdown__list {
  min-width: 0;
  display: grid;
  gap: 5px;
}

.quota-breakdown__row {
  min-width: 0;
  display: grid;
  grid-template-columns: 8px minmax(0, 1fr) auto;
  gap: 6px;
  align-items: center;
  color: var(--text-muted);
  font-size: 11px;
}

.quota-breakdown__row i {
  width: 8px;
  height: 8px;
  border-radius: var(--radius-pill);
}

.quota-breakdown__row span,
.quota-breakdown__row strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quota-breakdown__row strong {
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.quota-backend-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-width: 0;
  padding: 0 2px;
  color: var(--text-muted);
  font-size: 12px;
}

.quota-backend-row span,
.quota-backend-row strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quota-backend-row strong {
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.quota-recent {
  display: grid;
  gap: 1px;
}

.quota-recent__row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  gap: 12px;
  align-items: center;
  min-height: 32px;
  padding: 6px 8px;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  font-size: 12px;
}

.quota-recent__row:hover {
  background: var(--bg-hover);
}

.quota-recent__main {
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.quota-recent__main strong {
  color: var(--text);
}

.quota-recent__main span,
.quota-recent__row > span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.quota-empty {
  padding: 28px 12px;
  color: var(--text-muted);
  text-align: center;
  font-size: 13px;
}

.quota-empty--compact {
  padding: 12px;
}

@media (max-width: 860px) {
  .quota-toolbar {
    align-items: flex-start;
  }

  .quota-metrics {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .quota-metric--cost {
    grid-column: 1 / -1;
  }

  .quota-official__grid,
  .quota-official__credits,
  .quota-official-usage__metrics {
    grid-template-columns: 1fr;
  }

  .quota-recent__row {
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .quota-recent__row > span:last-child {
    display: none;
  }

  .quota-chart {
    height: 220px;
  }

  .quota-breakdown__body {
    grid-template-columns: 74px minmax(0, 1fr);
  }

  .quota-breakdown__chart {
    width: 74px;
    height: 74px;
  }
}
</style>

