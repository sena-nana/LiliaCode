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
  if (!window) return "暂无数据";
  return `剩余 ${formatPercent(100 - window.usedPercent)}`;
}
