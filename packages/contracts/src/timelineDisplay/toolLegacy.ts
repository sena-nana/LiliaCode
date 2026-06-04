import {
  deriveLiliaToolDisplay,
  getLiliaToolRule,
  readFirstString,
} from "../liliaTools.mjs";
import { normalizeClaudeTool } from "../claudeTools.mjs";
import type {
  AgentTimelineDisplay,
  AgentTimelineEventStatus,
} from "../timeline";
import { asString } from "./detailHelpers";

export function tryDeriveToolDisplay(
  kind: string,
  payload: Record<string, unknown>,
  title: string,
  summary: string,
  status: AgentTimelineEventStatus,
  isToolWindowKind: (kind: string) => boolean,
): AgentTimelineDisplay | null {
  if (getLiliaToolRule(kind, asString(payload.subkind))) {
    const display = deriveLiliaToolDisplay({
      kind,
      subkind: asString(payload.subkind),
      payload,
      title,
      status,
    });
    return finishToolDisplay(display, title, summary);
  }

  return tryDeriveLegacyClaudeToolDisplay(kind, payload, title, summary, status, isToolWindowKind);
}

function tryDeriveLegacyClaudeToolDisplay(
  kind: string,
  payload: Record<string, unknown>,
  title: string,
  summary: string,
  status: AgentTimelineEventStatus,
  isToolWindowKind: (kind: string) => boolean,
): AgentTimelineDisplay | null {
  if (!isLegacyToolKind(kind, isToolWindowKind)) return null;
  const toolName = readFirstString(payload, ["toolName", "tool", "name"], 200);
  if (!toolName) return null;

  const normalized = normalizeClaudeTool(toolName, payload.input, payload);
  if (normalized.kind === "tool" && kind !== "tool") return null;

  const display = deriveLiliaToolDisplay({
    kind: normalized.kind,
    subkind: normalized.subkind,
    payload: legacyToolPayload(normalized.payload, payload),
    title,
    status,
  });
  return finishToolDisplay(display, title, summary);
}

function legacyToolPayload(
  normalized: Record<string, unknown>,
  original: Record<string, unknown>,
): Record<string, unknown> {
  const allowedOriginalFields = [
    "approved",
    "cancelled",
    "decision",
    "duration",
    "error",
    "exit",
    "exitCode",
    "message",
    "output",
    "result",
    "stderr",
    "stdout",
    "structuredContent",
  ];
  const merged: Record<string, unknown> = { ...normalized };
  for (const key of allowedOriginalFields) {
    if (!(key in original)) continue;
    const value = original[key];
    if (key === "approved" && value === null) {
      merged[key] = value;
      continue;
    }
    if (value === undefined || value === null || value === "") continue;
    merged[key] = value;
  }
  return merged;
}

function finishToolDisplay(
  display: AgentTimelineDisplay | null,
  title: string,
  summary: string,
): AgentTimelineDisplay | null {
  if (!display) return null;
  const preview = summary || display.preview || title;
  const object = display.object?.trim() ? display.object : title;
  return {
    ...display,
    object,
    preview,
  };
}

function isLegacyToolKind(kind: string, isToolWindowKind: (kind: string) => boolean): boolean {
  return isToolWindowKind(kind) || kind === "plan";
}
