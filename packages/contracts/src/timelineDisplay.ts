import { compactLine, readRecord } from "./liliaTools.mjs";
import type {
  AgentTimelineDisplay,
  AgentTimelineDisplayListItem,
  AgentTimelineEventStatus,
  AgentTimelinePayload,
} from "./timeline";
import { applyDisplayContext, createDisplayContext } from "./timelineDisplay/context";
import { cleanDisplay, fallbackDisplay } from "./timelineDisplay/detailHelpers";
import { buildByKind } from "./timelineDisplay/kindDisplay";
import { tryDeriveToolDisplay } from "./timelineDisplay/toolLegacy";

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
    isAgentTimelineToolWindowKind,
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
  "subagent",
]);

export function isAgentTimelineToolWindowKind(kind: string): boolean {
  return AGENT_TIMELINE_TOOL_WINDOW_KINDS.has(kind);
}

export type { AgentTimelineDisplayListItem };
