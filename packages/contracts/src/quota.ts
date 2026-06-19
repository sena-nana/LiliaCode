import type { ChatBackendKind } from "./chat";

export type QuotaUsageStatsDays = 7 | 30;
export type QuotaUsageStatsBackendFilter = "all" | ChatBackendKind;

export interface QuotaUsageStatsInput {
  days?: QuotaUsageStatsDays;
  backend?: QuotaUsageStatsBackendFilter;
}

export type QuotaUsageQueryScope =
  | "summary"
  | "projects"
  | "conversations"
  | "tools"
  | "all";

export interface QuotaUsageQueryInput extends QuotaUsageStatsInput {
  scope?: QuotaUsageQueryScope;
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

export interface QuotaUsageProjectSummary extends QuotaUsageTokenTotals {
  projectId: string | null;
  projectName: string;
  projectCwd: string | null;
  knownCostUsd: number | null;
  costRecordCount: number;
  recordCount: number;
}

export interface QuotaUsageConversationSummary extends QuotaUsageTokenTotals {
  taskId: string;
  taskTitle: string;
  taskStatus: string;
  projectId: string | null;
  projectName: string | null;
  knownCostUsd: number | null;
  costRecordCount: number;
  recordCount: number;
}

export interface QuotaUsageToolSummary {
  key: string;
  label: string;
  kind: string;
  subkind: string | null;
  toolName: string | null;
  callCount: number;
  sharePercent: number;
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
  projects: QuotaUsageProjectSummary[];
  conversations: QuotaUsageConversationSummary[];
  tools: QuotaUsageToolSummary[];
}

export interface CodexAccountQuotaWindow {
  usedPercent: number;
  windowDurationMins: number | null;
  resetsAt: number | null;
}

export interface CodexAccountQuotaCredits {
  hasCredits: boolean;
  unlimited: boolean;
  balance: string | null;
}

export interface CodexRateLimitResetCredits {
  availableCount: number;
}

export interface CodexAccountUsageSummary {
  lifetimeTokens: number | null;
  peakDailyTokens: number | null;
  longestRunningTurnSec: number | null;
  currentStreakDays: number | null;
  longestStreakDays: number | null;
}

export interface CodexAccountUsageDailyBucket {
  startDate: string;
  tokens: number;
}

export interface CodexAccountUsage {
  summary: CodexAccountUsageSummary;
  dailyUsageBuckets: CodexAccountUsageDailyBucket[] | null;
}

export interface CodexAccountQuotaStatus {
  available: boolean;
  connectionMode: string;
  limitId: string | null;
  limitName: string | null;
  planType: string | null;
  rateLimitReachedType: string | null;
  fiveHour: CodexAccountQuotaWindow | null;
  weekly: CodexAccountQuotaWindow | null;
  sparkFiveHour: CodexAccountQuotaWindow | null;
  sparkWeekly: CodexAccountQuotaWindow | null;
  credits: CodexAccountQuotaCredits | null;
  sparkCredits: CodexAccountQuotaCredits | null;
  rateLimitResetCredits: CodexRateLimitResetCredits | null;
  accountUsage: CodexAccountUsage | null;
  usageError: string | null;
  fetchedAt: number;
  error: string | null;
}

export interface CodexRateLimitResetCreditConsumeInput {
  idempotencyKey: string;
}

export type CodexRateLimitResetCreditConsumeOutcome =
  | "reset"
  | "alreadyRedeemed"
  | "nothingToReset"
  | "noCredit";

export interface CodexRateLimitResetCreditConsumeResult {
  outcome: CodexRateLimitResetCreditConsumeOutcome;
  status: CodexAccountQuotaStatus;
}
