import { compactLine, deriveLiliaToolDisplay, readRecord } from "./liliaTools.mjs";
import {
  agentTimelinePayloadRecord,
  AGENT_TIMELINE_TOOL_WINDOW_KINDS,
  isAgentTimelineRunningStatus,
  TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT,
  TIMELINE_DISPLAY_TITLE_TEXT_LIMIT,
  type AgentTimelineEvent,
  type AgentTimelineDisplay,
  type AgentTimelineDisplayListItem,
  type AgentTimelineEventStatus,
  type AgentTimelinePayload,
} from "./timeline";
import { ASK_USER_INTERACTION_KIND } from "./agent-interaction";
import { applyDisplayContext, createDisplayContext } from "./timelineDisplay/context";
import { cleanDisplay, fallbackDisplay } from "./timelineDisplay/detailHelpers";
import { buildByKind } from "./timelineDisplay/kindDisplay";

export interface TimelineDisplayInput {
  kind: string;
  status: AgentTimelineEventStatus;
  title: string;
  summary: string | null;
  payload: AgentTimelinePayload;
  projectCwd?: string | null;
}

export interface TimelineDeclaredGroupUnit {
  key: string;
  count: number;
  unit: string | null;
}

type TimelineTimedEvent = Pick<AgentTimelineEvent, "createdAt" | "updatedAt">;
type TimelineProcessEvent = TimelineDisplayInput & TimelineTimedEvent;
type TimelineSubagentEvent = Pick<
  AgentTimelineEvent,
  "kind" | "status" | "title" | "summary" | "payload"
>;

const PROCESS_CATEGORY_LABELS: Record<string, string> = {
  command: "命令执行",
  file: "文件处理",
  mcp: "MCP 调用",
  plan: "计划更新",
  search: "搜索",
  subagent: "子代理任务",
  todo: "待办更新",
  tool: "工具调用",
  [ASK_USER_INTERACTION_KIND]: "用户提问",
};

export function deriveTimelineDisplay(input: TimelineDisplayInput): AgentTimelineDisplay {
  const kind = input.kind || "tool";
  const title = compactLine(input.title, TIMELINE_DISPLAY_TITLE_TEXT_LIMIT) || kind;
  const summary = compactLine(input.summary ?? "", TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT);
  const payload = readRecord(input.payload);
  const context = createDisplayContext(input.projectCwd);

  const subkind = typeof payload.subkind === "string"
    ? payload.subkind
    : kind === "tool" && (payload.hookName || payload.hookEvent)
      ? "hook"
      : null;
  const toolDisplay = deriveLiliaToolDisplay({
    kind,
    subkind,
    payload,
    title,
    status: input.status,
  });
  const display = toolDisplay
    ? cleanDisplay(finishLiliaToolDisplay(toolDisplay, title, summary))
    : cleanDisplay(buildByKind({ kind, status: input.status, title, summary, payload }));
  return applyDisplayContext(
    display ?? fallbackDisplay(kind, title, summary),
    context,
  );
}

export function timelineDeclaredGroupUnit(
  input: TimelineDisplayInput,
): TimelineDeclaredGroupUnit | null {
  return timelineDeclaredGroupUnitFromDisplay(deriveTimelineDisplay(input));
}

export function timelineDeclaredGroupUnitFromDisplay(
  display: Pick<AgentTimelineDisplay, "group">,
): TimelineDeclaredGroupUnit | null {
  const group = display.group;
  const key = group?.bucket?.trim() || group?.key?.trim();
  if (!key) return null;
  const count = typeof group?.count === "number" && Number.isFinite(group.count) && group.count > 0
    ? group.count
    : 1;
  return {
    key,
    count,
    unit: group?.unit?.trim() || null,
  };
}

export function timelineEventLabelFromDisplay(
  display: AgentTimelineDisplay,
  status: AgentTimelineEventStatus,
): string {
  const label = display.label?.trim();
  if (label) return label;
  const action = display.action?.trim();
  if (!action) return "事件";
  const verb = timelineActionStatusLabel(status, action);
  if (display.objectInLabel) {
    const object = display.object?.trim() ?? "";
    if (object) return `${verb} ${object}`;
  }
  return verb;
}

export function timelineGroupLabelFromDisplay(
  display: Pick<AgentTimelineDisplay, "action" | "group" | "label">,
  count: number,
  status: AgentTimelineEventStatus,
): string {
  const group = display.group;
  if (group?.key) {
    const unit = group.unit?.trim() || "项";
    const action = display.action?.trim();
    if (action) return `${timelineActionStatusLabel(status, action)} ${count} ${unit}`;
    const label = display.label?.trim() || "事件";
    return `${label} ${count} ${unit}`;
  }

  return `事件 ${count} 项`;
}

export function timelineActionStatusLabel(
  status: AgentTimelineEventStatus,
  verb: string,
): string {
  switch (status) {
    case "pending":
    case "started":
    case "running":
    case "in_progress":
      return `正在${verb}`;
    case "failed":
    case "error":
      return `${verb}失败`;
    case "cancelled":
      return `已取消${verb}`;
    case "skipped":
      return `已跳过${verb}`;
    case "info":
    case "requires_action":
    case "completed":
    case "done":
    case "success":
    default:
      return `已${verb}`;
  }
}

export function timelineLatestEventTimeMs(
  events: ReadonlyArray<TimelineTimedEvent>,
): number | null {
  const last = events[events.length - 1];
  if (!last) return null;
  const updatedAt = Number.isFinite(last.updatedAt) ? last.updatedAt : last.createdAt;
  const time = Math.max(last.createdAt, updatedAt);
  return Number.isFinite(time) ? time : null;
}

export function timelineThinkingDurationLabel(previousAt: number | null, now: number): string {
  if (previousAt === null || !Number.isFinite(now)) return "思考中";
  const seconds = Math.max(1, Math.floor((now - previousAt) / 1000));
  if (seconds < 60) return `思考中 ${seconds} 秒`;
  const minutes = Math.floor(seconds / 60);
  return `思考中 ${minutes} 分 ${seconds % 60} 秒`;
}

export function timelineRunningSubagentLabel(
  events: ReadonlyArray<TimelineSubagentEvent>,
): string {
  for (let i = events.length - 1; i >= 0; i -= 1) {
    const event = events[i];
    if (event.kind !== "subagent" || !isAgentTimelineRunningStatus(event.status)) continue;
    const payload = agentTimelinePayloadRecord(event) ?? {};
    const name = stringField(payload.subagentType) ||
      stringField(payload.agentType) ||
      event.title.trim();
    return `子代理${name || "任务"}${subagentAction(event, payload)}`;
  }
  return "";
}

export function timelineProcessEventsSummary(
  events: ReadonlyArray<TimelineProcessEvent>,
  finalEvent: Pick<AgentTimelineEvent, "createdAt">,
): string {
  const labels: string[] = [];
  const seen = new Set<string>();
  for (const event of events) {
    const label = timelineProcessEventCategoryLabel(event);
    if (!label || seen.has(label)) continue;
    seen.add(label);
    labels.push(label);
  }
  const duration = formatProcessDuration(processEventsElapsedMs(events, finalEvent));
  if (labels.length > 0) return [labels.join("、"), duration].filter(Boolean).join(" · ");
  if (duration) return `已处理 ${duration}`;
  return "处理中";
}

function timelineProcessEventCategoryLabel(event: TimelineDisplayInput): string {
  const declared = timelineDeclaredGroupUnit(event);
  if (!declared) return "";
  const key = declared.key ?? event.kind;
  return PROCESS_CATEGORY_LABELS[key] ?? PROCESS_CATEGORY_LABELS[event.kind] ?? "";
}

function processEventsElapsedMs(
  events: ReadonlyArray<TimelineTimedEvent>,
  finalEvent: Pick<AgentTimelineEvent, "createdAt">,
): number | null {
  let start = Number.POSITIVE_INFINITY;
  let processEnd = Number.NEGATIVE_INFINITY;
  let hasProcessDuration = false;
  for (const event of events) {
    if (!Number.isFinite(event.createdAt)) continue;
    const updatedAt = Number.isFinite(event.updatedAt) ? event.updatedAt : event.createdAt;
    start = Math.min(start, event.createdAt);
    processEnd = Math.max(processEnd, updatedAt, event.createdAt);
    if (updatedAt > event.createdAt) hasProcessDuration = true;
  }
  const end = hasProcessDuration || !Number.isFinite(finalEvent.createdAt)
    ? processEnd
    : Math.max(processEnd, finalEvent.createdAt);
  if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) return null;
  return end - start;
}

function formatProcessDuration(elapsedMs: number | null): string {
  if (elapsedMs === null) return "";
  return `${Math.max(1, Math.ceil(elapsedMs / 1000))} 秒`;
}

function subagentAction(
  event: TimelineSubagentEvent,
  payload: Record<string, unknown>,
): string {
  if (stringField(payload.lastToolName)) return "正在调用工具";
  const summary = (event.summary ?? "").trim();
  if (!summary) return "运行中";
  return summary.startsWith("正在") ? summary : `正在${summary}`;
}

function stringField(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function finishLiliaToolDisplay(
  display: AgentTimelineDisplay,
  title: string,
  summary: string,
): AgentTimelineDisplay {
  const object = display.object?.trim() ? display.object : title;
  return {
    ...display,
    object,
    preview: summary || display.preview || title,
  };
}

const AGENT_TIMELINE_TOOL_WINDOW_KIND_SET = new Set(AGENT_TIMELINE_TOOL_WINDOW_KINDS);

export function isAgentTimelineToolWindowKind(kind: string): boolean {
  return AGENT_TIMELINE_TOOL_WINDOW_KIND_SET.has(kind);
}

export type { AgentTimelineDisplayListItem };
