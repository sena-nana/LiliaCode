import { z } from "zod/v4";
import { isRecord, stringOrNull } from "./utils.mjs";

export const LILIA_QUOTA_TOOL_NAMES = new Set([
  "QueryQuotaUsage",
  "query_quota_usage",
  "mcp__lilia__query_quota_usage",
]);

export function isLiliaQuotaTool(toolName) {
  return LILIA_QUOTA_TOOL_NAMES.has(String(toolName || ""));
}

export const queryQuotaUsageInputSchema = {
  days: z.union([z.literal(7), z.literal(30)]).optional().default(7),
  backend: z.enum(["all", "claude", "codex"]).optional().default("all"),
  scope: z.enum(["summary", "projects", "conversations", "tools", "all"]).optional().default("all"),
};

export const queryQuotaUsageJsonSchema = {
  type: "object",
  properties: {
    days: { type: "integer", enum: [7, 30], default: 7 },
    backend: { type: "string", enum: ["all", "claude", "codex"], default: "all" },
    scope: {
      type: "string",
      enum: ["summary", "projects", "conversations", "tools", "all"],
      default: "all",
    },
  },
  additionalProperties: false,
};

export const codexQueryQuotaUsageDynamicTool = {
  name: "QueryQuotaUsage",
  description: "Query Lilia quota usage summaries through the Lilia internal quota plugin.",
  inputSchema: queryQuotaUsageJsonSchema,
};

function normalizeQuotaUsageInput(input) {
  const row = isRecord(input) ? input : {};
  return {
    days: row.days === 30 ? 30 : 7,
    backend: ["all", "claude", "codex"].includes(stringOrNull(row.backend))
      ? stringOrNull(row.backend)
      : "all",
    scope: ["summary", "projects", "conversations", "tools", "all"].includes(stringOrNull(row.scope))
      ? stringOrNull(row.scope)
      : "all",
  };
}

export function createQuotaUsageHandler(requestQuotaUsage) {
  return async function handleQueryQuotaUsage(input) {
    if (typeof requestQuotaUsage !== "function") {
      return {
        ok: false,
        error: "Lilia internal quota plugin is not available.",
      };
    }
    const payload = normalizeQuotaUsageInput(input);
    const response = await requestQuotaUsage(payload);
    if (response?.ok !== true) {
      return {
        ok: false,
        error: response?.error || "Lilia internal quota plugin failed.",
      };
    }
    return response.result ?? {
      ok: false,
      error: "Lilia internal quota plugin returned no result.",
    };
  };
}
