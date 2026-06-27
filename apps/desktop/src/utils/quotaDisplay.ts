import {
  clampPercent,
  codexAccountQuotaPercentLabel,
  codexAccountQuotaWindowRemainingLabel,
  createLiliaCodeCoreCodexQuotaUnavailableStatus,
  type ConnectionMode,
  type CodexAccountQuotaStatus,
} from "@lilia/contracts";

export { clampPercent };

export function quotaPercentTone(value: number): "normal" | "warn" | "error" {
  const percent = clampPercent(value);
  if (percent >= 100) return "error";
  if (percent >= 85) return "warn";
  return "normal";
}

export function formatPercent(value: number): string {
  return codexAccountQuotaPercentLabel(value);
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

export function quotaWindowLabel(window: { usedPercent: number } | null | undefined): string {
  return codexAccountQuotaWindowRemainingLabel(window);
}

export function codexQuotaUnavailableStatus(
  error: unknown,
  connectionMode: ConnectionMode = "codex-account",
): CodexAccountQuotaStatus {
  return createLiliaCodeCoreCodexQuotaUnavailableStatus({ error, connectionMode });
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
