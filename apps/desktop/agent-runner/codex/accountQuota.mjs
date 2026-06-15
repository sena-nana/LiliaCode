import { createCodexAppServer } from "./appServer.mjs";
import { initializeCodexAppServer } from "./runCodex.mjs";
import { isRecord, stringOrNull } from "../utils.mjs";

const FIVE_HOUR_MINUTES = 300;
const WEEKLY_MINUTES = 10080;
const FIVE_HOUR_TOLERANCE = 90;
const WEEKLY_TOLERANCE = 1440;

function numberOrNull(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function normalizeWindow(value) {
  if (!isRecord(value)) return null;
  const usedPercent = numberOrNull(value.usedPercent);
  if (usedPercent === null) return null;
  return {
    usedPercent,
    windowDurationMins: numberOrNull(value.windowDurationMins),
    resetsAt: numberOrNull(value.resetsAt),
  };
}

function durationNear(window, target, tolerance) {
  return typeof window?.windowDurationMins === "number" &&
    Math.abs(window.windowDurationMins - target) <= tolerance;
}

function normalizeRateLimitSnapshot(value) {
  if (!isRecord(value)) return null;
  const primary = normalizeWindow(value.primary);
  const secondary = normalizeWindow(value.secondary);
  const windows = [primary, secondary].filter(Boolean);
  const fiveHour = windows.find((window) =>
    durationNear(window, FIVE_HOUR_MINUTES, FIVE_HOUR_TOLERANCE)
  ) || primary;
  const weekly = windows.find((window) =>
    window !== fiveHour && durationNear(window, WEEKLY_MINUTES, WEEKLY_TOLERANCE)
  ) || (secondary !== fiveHour ? secondary : null);
  return {
    available: Boolean(fiveHour || weekly),
    connectionMode: "codex-account",
    limitId: stringOrNull(value.limitId),
    limitName: stringOrNull(value.limitName),
    planType: stringOrNull(value.planType),
    rateLimitReachedType: stringOrNull(value.rateLimitReachedType),
    fiveHour,
    weekly,
    fetchedAt: Date.now(),
    error: null,
  };
}

function readCodexLimitSnapshot(result) {
  if (!isRecord(result)) return null;
  const byLimitId = isRecord(result.rateLimitsByLimitId) ? result.rateLimitsByLimitId : null;
  if (byLimitId && isRecord(byLimitId.codex)) return byLimitId.codex;
  return isRecord(result.rateLimits) ? result.rateLimits : null;
}

export async function readCodexAccountQuotaStatus(
  { createServer = createCodexAppServer } = {},
) {
  const server = createServer();
  try {
    await initializeCodexAppServer(server);
    const result = await server.request("account/rateLimits/read");
    const snapshot = normalizeRateLimitSnapshot(readCodexLimitSnapshot(result));
    if (snapshot) return snapshot;
    return {
      available: false,
      connectionMode: "codex-account",
      limitId: null,
      limitName: null,
      planType: null,
      rateLimitReachedType: null,
      fiveHour: null,
      weekly: null,
      fetchedAt: Date.now(),
      error: "Codex 官方额度接口未返回可识别的额度数据。",
    };
  } finally {
    server.close();
  }
}
