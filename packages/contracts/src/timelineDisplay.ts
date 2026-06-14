import {
  compactLine,
  deriveLiliaToolDisplay,
  getLiliaToolRule,
  readRecord,
} from "./liliaTools.mjs";
import type {
  AgentTimelineDisplay,
  AgentTimelineDisplayListItem,
  AgentTimelineEventStatus,
  AgentTimelinePayload,
} from "./timeline";
import { applyDisplayContext, createDisplayContext } from "./timelineDisplay/context";
import { asString, cleanDisplay, fallbackDisplay } from "./timelineDisplay/detailHelpers";
import { buildByKind } from "./timelineDisplay/kindDisplay";

export interface TimelineDisplayInput {
  kind: string;
  status: AgentTimelineEventStatus;
  title: string;
  summary: string | null;
  payload: AgentTimelinePayload;
  projectCwd?: string | null;
}

export function deriveTimelineDisplay(input: TimelineDisplayInput): AgentTimelineDisplay {
  const kind = input.kind || "tool";
  const title = compactLine(input.title, 200) || kind;
  const summary = compactLine(input.summary ?? "", 1200);
  const payload = readRecord(input.payload);
  const context = createDisplayContext(input.projectCwd);

  const toolDisplay = tryDeriveToolDisplay(
    kind,
    payload,
    title,
    summary,
    input.status,
  );
  const display = toolDisplay
    ? cleanDisplay(toolDisplay)
    : cleanDisplay(buildByKind({ kind, status: input.status, title, summary, payload }));
  return applyDisplayContext(
    display ?? fallbackDisplay(kind, title, summary),
    context,
  );
}

const AGENT_TIMELINE_TOOL_WINDOW_KINDS = new Set([
  "tool",
  "command",
  "mcp",
  "search",
  "web_search",
  "web_fetch",
  "file_read",
  "file_change",
  "todo_list",
  "architecture_change",
  "subagent",
]);

export function isAgentTimelineToolWindowKind(kind: string): boolean {
  return AGENT_TIMELINE_TOOL_WINDOW_KINDS.has(kind);
}

function tryDeriveToolDisplay(
  kind: string,
  payload: Record<string, unknown>,
  title: string,
  summary: string,
  status: AgentTimelineEventStatus,
): AgentTimelineDisplay | null {
  const subkind = asString(payload.subkind);
  if (!getLiliaToolRule(kind, subkind)) return null;
  const display = deriveLiliaToolDisplay({
    kind,
    subkind,
    payload,
    title,
    status,
  });
  if (!display) return null;
  return {
    ...display,
    object: display.object?.trim() ? display.object : title,
    preview: summary || display.preview || title,
  };
}

export type { AgentTimelineDisplayListItem };
