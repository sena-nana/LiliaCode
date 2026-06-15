<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  AlertTriangle,
  Coins,
  Database,
  Gauge,
  RefreshCw,
} from "lucide-vue-next";
import type {
  ChatBackendKind,
  CodexAccountQuotaStatus,
  CodexAccountQuotaWindow,
  QuotaUsageDailyBucket,
  QuotaUsageStats,
  QuotaUsageStatsBackendFilter,
  QuotaUsageStatsDays,
} from "@lilia/contracts";
import { getCodexAccountQuotaStatus, getQuotaUsageStats } from "../../services/chat";
import {
  formatDateTime,
  formatPercent,
  formatUnixSeconds,
  quotaWindowLabel,
} from "../../utils/quotaDisplay";

type BackendOption = { value: QuotaUsageStatsBackendFilter; label: string };
type DaysOption = { value: QuotaUsageStatsDays; label: string };
type OfficialQuotaWindowRow = {
  key: "fiveHour" | "weekly";
  label: string;
  window: CodexAccountQuotaWindow | null | undefined;
};

const backendOptions: BackendOption[] = [
  { value: "all", label: "全部" },
  { value: "codex", label: "Codex" },
  { value: "claude", label: "Claude" },
];
const daysOptions: DaysOption[] = [
  { value: 7, label: "7 天" },
  { value: 30, label: "30 天" },
];

const selectedBackend = ref<QuotaUsageStatsBackendFilter>("all");
const selectedDays = ref<QuotaUsageStatsDays>(7);
const stats = ref<QuotaUsageStats | null>(null);
const officialQuota = ref<CodexAccountQuotaStatus | null>(null);
const loading = ref(false);
const quotaLoading = ref(false);
const error = ref("");
let requestSeq = 0;
let quotaRequestSeq = 0;

const totalRecords = computed(() => stats.value?.cost.totalRecordCount ?? 0);
const hasUsage = computed(() => totalRecords.value > 0);
const totalTokens = computed(() => stats.value?.totals.totalTokens ?? 0);
const refreshing = computed(() => loading.value || quotaLoading.value);
const showOfficialQuota = computed(() =>
  officialQuota.value?.connectionMode === "codex-account",
);
const officialQuotaWindows = computed<OfficialQuotaWindowRow[]>(() => [
  { key: "fiveHour", label: "5 小时限额", window: officialQuota.value?.fiveHour },
  { key: "weekly", label: "周限额", window: officialQuota.value?.weekly },
]);
const hasOfficialQuotaWindow = computed(() => officialQuotaWindows.value.some((row) => row.window));
const maxDailyTokens = computed(() =>
  Math.max(1, ...(stats.value?.daily.map((bucket) => bucket.totalTokens) ?? [0])),
);
const sortedBackends = computed(() =>
  [...(stats.value?.backends ?? [])].sort((a, b) => b.totalTokens - a.totalTokens),
);
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

function backendLabel(backend: ChatBackendKind | QuotaUsageStatsBackendFilter) {
  if (backend === "codex") return "Codex";
  if (backend === "claude") return "Claude";
  return "全部";
}

function formatNumber(value: number) {
  return new Intl.NumberFormat("zh-CN").format(value);
}

function formatCompactNumber(value: number) {
  return new Intl.NumberFormat("zh-CN", {
    notation: "compact",
    maximumFractionDigits: value >= 1000 ? 1 : 0,
  }).format(value);
}

function formatCost(value: number) {
  return `$${value.toFixed(value >= 1 ? 2 : 4)}`;
}

function formatRecordCost(value: number | null) {
  return value === null ? "--" : formatCost(value);
}

function formatDay(value: number) {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
  }).format(new Date(value));
}

function chartX(index: number, count: number) {
  if (count <= 1) return 42;
  return 42 + index * (620 / (count - 1));
}

function barWidth(count: number) {
  return Math.max(6, Math.min(22, 520 / Math.max(1, count)));
}

function barHeight(tokens: number) {
  return Math.round((tokens / maxDailyTokens.value) * 112);
}

function stackSegments(bucket: QuotaUsageDailyBucket) {
  const max = Math.max(1, bucket.totalTokens);
  const input = barHeight(bucket.inputTokens);
  const output = barHeight(bucket.outputTokens);
  const cache = barHeight(bucket.cacheReadTokens + bucket.cacheCreationTokens);
  const normalized = [
    { key: "input", height: input, class: "quota-chart__segment--input" },
    { key: "output", height: output, class: "quota-chart__segment--output" },
    { key: "cache", height: cache, class: "quota-chart__segment--cache" },
  ].filter((item) => item.height > 0);
  if (bucket.totalTokens > 0 && normalized.length === 0) {
    const height = Math.max(2, barHeight(max));
    return [{ key: "total", height, y: 136 - height, class: "quota-chart__segment--input" }];
  }
  let y = 136;
  return normalized.map((item) => {
    y -= item.height;
    return { ...item, y };
  });
}

function backendPercent(tokens: number) {
  if (totalTokens.value <= 0) return 0;
  return Math.max(4, Math.round((tokens / totalTokens.value) * 100));
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
    if (seq === requestSeq) stats.value = result;
  } catch (err) {
    if (seq === requestSeq) error.value = String(err);
  } finally {
    if (seq === requestSeq) loading.value = false;
  }
}

async function loadOfficialQuota() {
  const seq = ++quotaRequestSeq;
  quotaLoading.value = true;
  try {
    const result = await getCodexAccountQuotaStatus();
    if (seq === quotaRequestSeq) officialQuota.value = result;
  } catch (err) {
    if (seq === quotaRequestSeq) {
      officialQuota.value = {
        available: false,
        connectionMode: "codex-account",
        limitId: null,
        limitName: null,
        planType: null,
        rateLimitReachedType: null,
        fiveHour: null,
        weekly: null,
        fetchedAt: Date.now(),
        error: String(err),
      };
    }
  } finally {
    if (seq === quotaRequestSeq) quotaLoading.value = false;
  }
}

async function refreshAll() {
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
  void refreshAll();
});
</script>

<template>
  <div class="card quota-panel">
    <div class="quota-panel__head">
      <h2>
        <span class="card-h2__title">
          <Gauge :size="14" aria-hidden="true" />
          额度统计
        </span>
      </h2>
      <button
        type="button"
        class="ui-button ui-button--ghost quota-panel__refresh"
        :disabled="refreshing"
        @click="refreshAll"
      >
        <RefreshCw :size="12" :class="{ 'is-spinning': refreshing }" aria-hidden="true" />
        刷新
      </button>
    </div>

    <div class="quota-panel__controls">
      <div class="ui-segmented" role="radiogroup" aria-label="额度统计后端">
        <button
          v-for="opt in backendOptions"
          :key="opt.value"
          type="button"
          role="radio"
          :aria-checked="selectedBackend === opt.value"
          :class="{ 'is-active': selectedBackend === opt.value }"
          :disabled="loading"
          @click="selectBackend(opt.value)"
        >
          {{ opt.label }}
        </button>
      </div>
      <div class="ui-segmented" role="radiogroup" aria-label="额度统计时间范围">
        <button
          v-for="opt in daysOptions"
          :key="opt.value"
          type="button"
          role="radio"
          :aria-checked="selectedDays === opt.value"
          :class="{ 'is-active': selectedDays === opt.value }"
          :disabled="loading"
          @click="selectDays(opt.value)"
        >
          {{ opt.label }}
        </button>
      </div>
    </div>

    <div v-if="showOfficialQuota" class="quota-official">
      <div class="quota-official__head">
        <div>
          <strong>Codex 官方额度</strong>
          <span>
            {{ officialQuota?.planType ? `计划 ${officialQuota.planType}` : "官方账号" }}
            <template v-if="officialQuota?.limitName"> · {{ officialQuota.limitName }}</template>
          </span>
        </div>
        <span class="ui-badge ui-badge--muted">
          {{ quotaLoading ? "读取中" : `查询 ${formatDateTime(officialQuota?.fetchedAt ?? Date.now())}` }}
        </span>
      </div>
      <div v-if="hasOfficialQuotaWindow" class="quota-official__grid">
        <div
          v-for="row in officialQuotaWindows"
          :key="row.key"
          class="quota-official-window"
        >
          <span>{{ row.label }}</span>
          <strong>{{ row.window ? formatPercent(row.window.usedPercent) : "--" }}</strong>
          <small>{{ quotaWindowLabel(row.window) }}</small>
          <small>重置 {{ formatUnixSeconds(row.window?.resetsAt) }}</small>
        </div>
      </div>
      <div v-else class="quota-official__empty">
        暂无官方额度数据
      </div>
      <div v-if="officialQuota?.error" class="quota-official__error">
        {{ officialQuota.error }}
      </div>
    </div>

    <div v-if="error" class="conn-banner conn-banner--err">
      <AlertTriangle :size="16" aria-hidden="true" />
      <div>
        <div class="conn-banner__title">额度统计</div>
        <div class="conn-banner__hint">{{ error }}</div>
      </div>
    </div>

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
        <span>缓存</span>
        <strong>
          {{ formatNumber((stats?.totals.cacheReadTokens ?? 0) + (stats?.totals.cacheCreationTokens ?? 0)) }}
        </strong>
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
      <span class="ui-badge ui-badge--muted">{{ backendLabel(selectedBackend) }} · 近 {{ selectedDays }} 天</span>
    </h2>

    <div v-if="!hasUsage && !loading" class="quota-empty">
      暂无新增额度数据
    </div>

    <div v-else class="quota-chart-wrap">
      <svg class="quota-chart" viewBox="0 0 704 170" role="img" aria-label="每日 Token 趋势">
        <line x1="34" y1="136" x2="684" y2="136" class="quota-chart__axis" />
        <g v-for="(bucket, index) in stats?.daily ?? []" :key="bucket.dayStart">
          <g
            v-for="segment in stackSegments(bucket)"
            :key="segment.key"
          >
            <rect
              :x="chartX(index, stats?.daily.length ?? 0) - barWidth(stats?.daily.length ?? 0) / 2"
              :y="segment.y"
              :width="barWidth(stats?.daily.length ?? 0)"
              :height="segment.height"
              rx="3"
              :class="segment.class"
            />
          </g>
          <text
            v-if="selectedDays === 7 || index % 4 === 0"
            :x="chartX(index, stats?.daily.length ?? 0)"
            y="157"
            text-anchor="middle"
            class="quota-chart__label"
          >
            {{ formatDay(bucket.dayStart) }}
          </text>
          <title>{{ formatDay(bucket.dayStart) }} · {{ formatNumber(bucket.totalTokens) }} tokens</title>
        </g>
      </svg>
      <div class="quota-chart__legend">
        <span><i class="quota-dot quota-dot--input" />输入</span>
        <span><i class="quota-dot quota-dot--output" />输出</span>
        <span><i class="quota-dot quota-dot--cache" />缓存</span>
      </div>
    </div>
  </div>

  <div class="card quota-panel">
    <h2>
      <span class="card-h2__title">
        <Coins :size="14" aria-hidden="true" />
        分布与记录
      </span>
    </h2>

    <div v-if="sortedBackends.length" class="quota-backends">
      <div
        v-for="row in sortedBackends"
        :key="row.backend"
        class="quota-backend-row"
      >
        <div class="quota-backend-row__head">
          <span>{{ backendLabel(row.backend) }}</span>
          <strong>{{ formatCompactNumber(row.totalTokens) }} tokens</strong>
        </div>
        <div class="quota-backend-row__track">
          <span :style="{ width: `${backendPercent(row.totalTokens)}%` }" />
        </div>
      </div>
    </div>

    <div v-if="stats?.recent.length" class="quota-recent">
      <div
        v-for="record in stats.recent"
        :key="record.eventId"
        class="quota-recent__row"
      >
        <div class="quota-recent__main">
          <strong>{{ backendLabel(record.backend) }}</strong>
          <span>{{ formatDateTime(record.createdAt) }}</span>
        </div>
        <span>{{ formatCompactNumber(record.totalTokens) }} tokens</span>
        <span>{{ formatRecordCost(record.knownCostUsd) }}</span>
      </div>
    </div>

    <div v-if="!hasUsage && !loading" class="quota-empty quota-empty--compact">
      暂无新增额度数据
    </div>
  </div>
</template>

<style scoped>
.quota-panel {
  display: grid;
  gap: 12px;
}

.quota-panel__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.quota-panel__head h2,
.quota-panel h2 {
  margin-bottom: 0;
}

.quota-panel__refresh {
  height: 28px;
  padding-inline: 8px;
  font-size: 12px;
}

.quota-panel__controls {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: 10px;
}

.quota-metrics {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
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
  padding: 10px;
  border: 1px solid var(--border-soft);
  border-radius: 8px;
  background: var(--bg);
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

.quota-official__head strong {
  color: var(--text);
  font-size: 13px;
}

.quota-official__head span,
.quota-official-window span,
.quota-official-window small,
.quota-official__empty,
.quota-official__error {
  overflow: hidden;
  color: var(--text-muted);
  font-size: 11px;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quota-official__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.quota-official-window {
  min-width: 0;
  display: grid;
  gap: 3px;
  padding: 9px 10px;
  border: 1px solid var(--border-soft);
  border-radius: 7px;
  background: var(--bg-subtle);
}

.quota-official-window strong {
  color: var(--accent);
  font-size: 18px;
  font-variant-numeric: tabular-nums;
  line-height: 1.15;
}

.quota-official__empty {
  padding: 8px 2px;
}

.quota-official__error {
  color: var(--warn);
}

.quota-chart-wrap {
  min-width: 0;
  border: 1px solid var(--border-soft);
  border-radius: 8px;
  background: var(--bg);
}

.quota-chart {
  display: block;
  width: 100%;
  height: 170px;
}

.quota-chart__axis {
  stroke: var(--border-strong);
  stroke-width: 1;
}

.quota-chart__label {
  fill: var(--text-muted);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}

.quota-chart__segment--input {
  fill: var(--accent);
}

.quota-chart__segment--output {
  fill: var(--ok);
}

.quota-chart__segment--cache {
  fill: var(--warn);
}

.quota-chart__legend {
  display: flex;
  gap: 14px;
  padding: 0 12px 10px;
  color: var(--text-muted);
  font-size: 12px;
}

.quota-chart__legend span {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.quota-dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
}

.quota-dot--input {
  background: var(--accent);
}

.quota-dot--output {
  background: var(--ok);
}

.quota-dot--cache {
  background: var(--warn);
}

.quota-backends {
  display: grid;
  gap: 8px;
}

.quota-backend-row {
  display: grid;
  gap: 6px;
}

.quota-backend-row__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  color: var(--text-muted);
  font-size: 12px;
}

.quota-backend-row__head strong {
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.quota-backend-row__track {
  height: 7px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--bg-subtle);
}

.quota-backend-row__track span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
}

.quota-recent {
  display: grid;
  gap: 1px;
  border-top: 1px solid var(--border-soft);
  padding-top: 8px;
}

.quota-recent__row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  gap: 12px;
  align-items: center;
  min-height: 32px;
  padding: 6px 8px;
  border-radius: 6px;
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
  .quota-metrics {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .quota-metric--cost {
    grid-column: 1 / -1;
  }

  .quota-official__grid {
    grid-template-columns: 1fr;
  }

  .quota-recent__row {
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .quota-recent__row > span:last-child {
    display: none;
  }
}
</style>
