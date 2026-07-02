import {
  type ConnectionMode,
  type CodexAccountQuotaStatus,
} from "@lilia/contracts";
import { createCodexQuotaUnavailableStatus } from "@lilia/contracts/liliaCodeCore.mjs";

export function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, value));
}

export function quotaPercentTone(value: number): "normal" | "warn" | "error" {
  const percent = clampPercent(value);
  if (percent >= 100) return "error";
  if (percent >= 85) return "warn";
  return "normal";
}

export function formatPercent(value: number): string {
  return `${new Intl.NumberFormat("zh-CN", {
    maximumFractionDigits: value >= 10 ? 0 : 1,
  }).format(clampPercent(value))}%`;
}

export function formatCompactNumber(value: number): string {
  return new Intl.NumberFormat("zh-CN", {
    notation: "compact",
    maximumFractionDigits: value >= 1000 ? 1 : 0,
  }).format(value);
}

export function formatDateTime(value: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

export function formatUnixSeconds(value: number | null | undefined): string {
  if (!value || value <= 0) return "--";
  return formatDateTime(value * 1000);
}

export function quotaWindowShortLabel(
  window: { windowDurationMins: number | null } | null | undefined,
): string {
  const mins = window?.windowDurationMins;
  if (!mins || mins <= 0) return "额度";
  if (mins % 1440 === 0) return `${mins / 1440}d`;
  if (mins % 60 === 0) return `${mins / 60}h`;
  return `${mins}m`;
}

export function quotaWindowRemainingPercent(
  window: { usedPercent: number } | null | undefined,
): number {
  return clampPercent(100 - (window?.usedPercent ?? 0));
}

export function quotaWindowLabel(window: { usedPercent: number } | null | undefined): string {
  if (!window) return "暂无数据";
  return `剩余 ${formatPercent(quotaWindowRemainingPercent(window))}`;
}

export function quotaWindowLine(
  window: { usedPercent: number; windowDurationMins: number | null } | null | undefined,
  suffix = "",
): string {
  if (!window) return "额度 · 暂无数据";
  const base = `${quotaWindowShortLabel(window)} · ${quotaWindowLabel(window)}`;
  return suffix ? `${base} · ${suffix}` : base;
}

export function codexQuotaUnavailableStatus(
  error: unknown,
  connectionMode: ConnectionMode = "codex-account",
): CodexAccountQuotaStatus {
  return createCodexQuotaUnavailableStatus({ error, connectionMode });
}

export function codexAccountNeedsLogin(
  status: CodexAccountQuotaStatus | null | undefined,
  supportsRequiredProtocol = false,
): boolean {
  if (!status || status.available) return false;
  const text = (status.error ?? "").toLowerCase();
  if (!text.trim()) return supportsRequiredProtocol;
  return /未登录|not logged|login|auth/.test(text);
}

