import { chatBackendLabel, type ChatBackendKind } from "./chat";
import type { ConnectionMode } from "./provider";
import {
  CODEX_RATE_LIMIT_RESET_CREDIT_CONSUME_OUTCOMES,
  CODEX_RATE_LIMIT_RESET_CREDIT_CONSUME_OUTCOME_LABELS,
  codexRateLimitResetCreditConsumeOutcomeLabel as codexRateLimitResetCreditConsumeOutcomeLabelImpl,
  DEFAULT_CODEX_RATE_LIMIT_RESET_CREDIT_CONSUME_OUTCOME_LABEL,
  DEFAULT_QUOTA_USAGE_QUERY_SCOPE,
  DEFAULT_QUOTA_USAGE_STATS_DAYS,
  isCodexRateLimitResetCreditConsumeOutcome as isCodexRateLimitResetCreditConsumeOutcomeImpl,
  isLiliaQuotaTool as isLiliaQuotaToolImpl,
  isQuotaUsageQueryScope as isQuotaUsageQueryScopeImpl,
  isQuotaUsageStatsBackendExtraFilter as isQuotaUsageStatsBackendExtraFilterImpl,
  isQuotaUsageStatsBackendFilter as isQuotaUsageStatsBackendFilterImpl,
  isQuotaUsageStatsDays as isQuotaUsageStatsDaysImpl,
  normalizeQuotaUsageQueryScope as normalizeQuotaUsageQueryScopeImpl,
  QUOTA_CONTRACT,
  QUOTA_USAGE_CLAUDE_TOOL_NAME,
  QUOTA_USAGE_CONSUME_CODEX_RATE_LIMIT_RESET_CREDIT_COMMAND,
  QUOTA_USAGE_GET_CODEX_ACCOUNT_STATUS_COMMAND,
  QUOTA_USAGE_GET_STATS_COMMAND,
  QUOTA_USAGE_MCP_TOOL_NAME,
  QUOTA_USAGE_QUERY_SCOPES,
  QUOTA_USAGE_STATS_BACKEND_EXTRA_FILTERS,
  QUOTA_USAGE_STATS_BACKEND_FILTER_LABELS,
  QUOTA_USAGE_STATS_BACKEND_FILTERS,
  QUOTA_USAGE_STATS_DAYS,
  QUOTA_USAGE_TOOL_NAME,
  QUOTA_USAGE_TOOL_NAMES,
  QUERY_QUOTA_USAGE_INPUT_SCHEMA,
  type CodexRateLimitResetCreditConsumeOutcome as ContractCodexRateLimitResetCreditConsumeOutcome,
  type QuotaUsageQueryScope as ContractQuotaUsageQueryScope,
  type QuotaUsageStatsBackendFilter as ContractQuotaUsageStatsBackendFilter,
  type QuotaUsageStatsDays as ContractQuotaUsageStatsDays,
  type QuotaUsageToolName as ContractQuotaUsageToolName,
} from "./quotaContract.mjs";

export {
  CODEX_RATE_LIMIT_RESET_CREDIT_CONSUME_OUTCOMES,
  CODEX_RATE_LIMIT_RESET_CREDIT_CONSUME_OUTCOME_LABELS,
  DEFAULT_CODEX_RATE_LIMIT_RESET_CREDIT_CONSUME_OUTCOME_LABEL,
  DEFAULT_QUOTA_USAGE_QUERY_SCOPE,
  DEFAULT_QUOTA_USAGE_STATS_DAYS,
  QUOTA_CONTRACT,
  QUOTA_USAGE_CLAUDE_TOOL_NAME,
  QUOTA_USAGE_CONSUME_CODEX_RATE_LIMIT_RESET_CREDIT_COMMAND,
  QUOTA_USAGE_GET_CODEX_ACCOUNT_STATUS_COMMAND,
  QUOTA_USAGE_GET_STATS_COMMAND,
  QUOTA_USAGE_MCP_TOOL_NAME,
  QUOTA_USAGE_QUERY_SCOPES,
  QUOTA_USAGE_STATS_BACKEND_EXTRA_FILTERS,
  QUOTA_USAGE_STATS_BACKEND_FILTER_LABELS,
  QUOTA_USAGE_STATS_BACKEND_FILTERS,
  QUOTA_USAGE_STATS_DAYS,
  QUOTA_USAGE_TOOL_NAME,
  QUOTA_USAGE_TOOL_NAMES,
  QUERY_QUOTA_USAGE_INPUT_SCHEMA,
};

export type QuotaUsageStatsDays = ContractQuotaUsageStatsDays;
export type QuotaUsageStatsBackendExtraFilter =
  (typeof QUOTA_USAGE_STATS_BACKEND_EXTRA_FILTERS)[number];
export type QuotaUsageStatsBackendFilter = ContractQuotaUsageStatsBackendFilter;
export type QuotaUsageToolName = ContractQuotaUsageToolName;
export type CodexRateLimitResetCreditConsumeOutcome =
  ContractCodexRateLimitResetCreditConsumeOutcome;

export const isQuotaUsageStatsDays = isQuotaUsageStatsDaysImpl as (
  value: unknown,
) => value is QuotaUsageStatsDays;
export const isQuotaUsageStatsBackendExtraFilter =
  isQuotaUsageStatsBackendExtraFilterImpl as (
    value: unknown,
  ) => value is QuotaUsageStatsBackendExtraFilter;
export const isQuotaUsageStatsBackendFilter =
  isQuotaUsageStatsBackendFilterImpl as (
    value: unknown,
  ) => value is QuotaUsageStatsBackendFilter;

export function quotaUsageStatsBackendFilterLabel(
  backend: QuotaUsageStatsBackendFilter,
): string {
  return isQuotaUsageStatsBackendExtraFilter(backend)
    ? QUOTA_USAGE_STATS_BACKEND_FILTER_LABELS[backend]
    : chatBackendLabel(backend);
}

export function quotaUsageStatsDaysLabel(days: QuotaUsageStatsDays): string {
  return `${days} 天`;
}

export interface QuotaUsageStatsInput {
  days?: QuotaUsageStatsDays;
  backend?: QuotaUsageStatsBackendFilter;
}

export type QuotaUsageQueryScope = ContractQuotaUsageQueryScope;

export const isQuotaUsageQueryScope = isQuotaUsageQueryScopeImpl as (
  value: unknown,
) => value is QuotaUsageQueryScope;
export const normalizeQuotaUsageQueryScope = normalizeQuotaUsageQueryScopeImpl as (
  value: unknown,
) => QuotaUsageQueryScope;
export const isLiliaQuotaTool = isLiliaQuotaToolImpl as (
  toolName: unknown,
) => toolName is QuotaUsageToolName;

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

export function codexAccountQuotaWindowShortLabel(
  window: Pick<CodexAccountQuotaWindow, "windowDurationMins"> | null | undefined,
): string {
  const mins = window?.windowDurationMins;
  if (!mins || mins <= 0) return "额度";
  if (mins % 1440 === 0) return `${mins / 1440}d`;
  if (mins % 60 === 0) return `${mins / 60}h`;
  return `${mins}m`;
}

export function codexAccountQuotaPercentLabel(value: number): string {
  return `${new Intl.NumberFormat("zh-CN", {
    maximumFractionDigits: value >= 10 ? 0 : 1,
  }).format(clampPercent(value))}%`;
}

export function codexAccountQuotaWindowRemainingPercent(
  window: Pick<CodexAccountQuotaWindow, "usedPercent"> | null | undefined,
): number {
  return clampPercent(100 - (window?.usedPercent ?? 0));
}

export function codexAccountQuotaWindowRemainingLabel(
  window: Pick<CodexAccountQuotaWindow, "usedPercent"> | null | undefined,
): string {
  if (!window) return "暂无数据";
  return `剩余 ${codexAccountQuotaPercentLabel(codexAccountQuotaWindowRemainingPercent(window))}`;
}

export function codexAccountQuotaWindowRemainingLine(
  window: Pick<CodexAccountQuotaWindow, "usedPercent" | "windowDurationMins"> | null | undefined,
  suffix = "",
): string {
  if (!window) return "额度 · 暂无数据";
  const base = `${codexAccountQuotaWindowShortLabel(window)} · ${codexAccountQuotaWindowRemainingLabel(window)}`;
  return suffix ? `${base} · ${suffix}` : base;
}

export interface CodexAccountQuotaCredits {
  hasCredits: boolean;
  unlimited: boolean;
  balance: string | null;
}

export function codexAccountQuotaCreditsLabel(
  credits: CodexAccountQuotaCredits | null | undefined,
): string {
  if (!credits || !credits.hasCredits) return "暂无 credit 数据";
  if (credits.unlimited) return "不限";
  if (credits.balance) return `剩余 ${credits.balance}`;
  return "可用";
}

export function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, value));
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
  connectionMode: ConnectionMode;
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

export const isCodexRateLimitResetCreditConsumeOutcome =
  isCodexRateLimitResetCreditConsumeOutcomeImpl as (
    value: unknown,
  ) => value is CodexRateLimitResetCreditConsumeOutcome;

export const codexRateLimitResetCreditConsumeOutcomeLabel =
  codexRateLimitResetCreditConsumeOutcomeLabelImpl as (
    outcome: CodexRateLimitResetCreditConsumeOutcome | string,
  ) => string;

export interface CodexRateLimitResetCreditConsumeResult {
  outcome: CodexRateLimitResetCreditConsumeOutcome;
  status: CodexAccountQuotaStatus;
}
