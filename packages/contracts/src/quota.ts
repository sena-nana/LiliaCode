import type { ChatBackendKind } from "./chat";

export type QuotaUsageStatsDays = 7 | 30;
export type QuotaUsageStatsBackendFilter = "all" | ChatBackendKind;

export interface QuotaUsageStatsInput {
  days?: QuotaUsageStatsDays;
  backend?: QuotaUsageStatsBackendFilter;
}

export interface QuotaUsageTokenTotals {
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  totalTokens: number;
}

export interface QuotaUsageCostCoverage {
  knownCostUsd: number | null;
  costRecordCount: number;
  totalRecordCount: number;
}

export interface QuotaUsageDailyBucket extends QuotaUsageTokenTotals {
  dayStart: number;
  knownCostUsd: number | null;
  costRecordCount: number;
  recordCount: number;
}

export interface QuotaUsageBackendSummary extends QuotaUsageTokenTotals {
  backend: ChatBackendKind;
  knownCostUsd: number | null;
  costRecordCount: number;
  recordCount: number;
}

export interface QuotaUsageRecentRecord extends QuotaUsageTokenTotals {
  eventId: string;
  taskId: string;
  turnId: string | null;
  backend: ChatBackendKind;
  sessionId: string | null;
  knownCostUsd: number | null;
  createdAt: number;
}

export interface QuotaUsageStats {
  days: QuotaUsageStatsDays;
  backend: QuotaUsageStatsBackendFilter;
  rangeStart: number;
  rangeEnd: number;
  totals: QuotaUsageTokenTotals;
  cost: QuotaUsageCostCoverage;
  daily: QuotaUsageDailyBucket[];
  backends: QuotaUsageBackendSummary[];
  recent: QuotaUsageRecentRecord[];
}
