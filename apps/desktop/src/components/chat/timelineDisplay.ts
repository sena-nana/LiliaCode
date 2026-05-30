import {
  deriveTimelineDisplay,
  type AgentTimelineDisplay,
  type AgentTimelineEvent,
  type AgentTimelineEventStatus,
  type AgentTimelinePayload,
} from "@lilia/contracts";

export type MarkdownBlockTone = "default" | "muted";

const TIMELINE_SUMMARY_MAX_LENGTH = 220;

export type TimelinePayloadRecord = Record<string, AgentTimelinePayload | undefined>;

export interface TimelineMarkdownView {
  content: string;
  tone: MarkdownBlockTone;
  singleLine: boolean;
}

interface TimelineDeclaredGroupUnit {
  key: string;
  count: number;
  unit: string | null;
}

export interface TimelineDisplayContext {
  projectCwd?: string | null;
  activePlanApprovalTurnId?: string | null;
}

const RUNNING_STATUSES = new Set<AgentTimelineEventStatus>([
  "pending",
  "started",
  "running",
  "in_progress",
]);

type DisplayDerivableEvent = Pick<
  AgentTimelineEvent,
  "kind" | "status" | "title" | "summary" | "payload"
> & Partial<Pick<AgentTimelineEvent, "turnId">>;

export function readTimelinePayloadRecord(
  event: Pick<AgentTimelineEvent, "payload">,
): TimelinePayloadRecord {
  return readPayloadRecord(event.payload);
}

/**
 * Display 是从 `{kind, status, title, summary, payload}` 现算的视图缓存，
 * 不读 DB 上的旧 `display` 列，也不允许任何 caller 自己拼。换 display 规则
 * 时历史事件自动跟着变 —— 这是把 display 移出 DB 的全部动机。
 */
export function readTimelineDisplay(
  event: DisplayDerivableEvent,
  context: TimelineDisplayContext = {},
): AgentTimelineDisplay {
  return deriveTimelineDisplay({
    kind: event.kind,
    status: event.status,
    title: event.title,
    summary: event.summary,
    payload: event.payload,
    projectCwd: context.projectCwd,
  });
}

function readPayloadRecord(payload: unknown): TimelinePayloadRecord {
  return payload && typeof payload === "object" && !Array.isArray(payload)
    ? payload as TimelinePayloadRecord
    : {};
}

export function timelineFinalText(event: Pick<AgentTimelineEvent, "kind" | "payload">): string {
  if (event.kind !== "message") return "";
  const payload = readTimelinePayloadRecord(event);
  if (payload.role !== "assistant") return "";
  const content = payload.content;
  return typeof content === "string" ? content : "";
}

/** 「最终回复」= assistant message。流式中（status=running）也算，避免 token
 * 增量到达时组件树展开/折叠状态抖动。 */
export function isTimelineFinalReply(
  event: Pick<AgentTimelineEvent, "kind" | "payload">,
): boolean {
  if (event.kind !== "message") return false;
  return readTimelinePayloadRecord(event).role === "assistant";
}

export function isTimelineInterruptEvent(
  event: Pick<AgentTimelineEvent, "kind" | "payload">,
): boolean {
  return event.kind === "error" && readTimelinePayloadRecord(event).interrupted === true;
}

/**
 * runner 会保留恢复/调试用的内部事件；主时间线只展示用户可操作的过程。
 */
export function isHiddenTimelineEvent(
  event: Pick<AgentTimelineEvent, "kind">,
): boolean {
  return event.kind === "turn" || event.kind === "reasoning";
}

export function isTimelineFinalReplyStreaming(
  event: Pick<AgentTimelineEvent, "kind" | "payload" | "status">,
): boolean {
  return isTimelineFinalReply(event) && RUNNING_STATUSES.has(event.status);
}

function timelineDefaultExpanded(
  event: DisplayDerivableEvent,
  context: TimelineDisplayContext = {},
): boolean {
  if (event.kind === "plan" && readTimelinePayloadRecord(event).approved === null) {
    return Boolean(
      context.activePlanApprovalTurnId &&
      event.turnId === context.activePlanApprovalTurnId
    );
  }
  const display = readTimelineDisplay(event, context);
  if (typeof display?.defaultExpanded === "boolean") return display.defaultExpanded;
  return isTimelineFinalReply(event);
}

export function isTimelineExpanded(
  event: DisplayDerivableEvent & Pick<AgentTimelineEvent, "id">,
  toggledIds: ReadonlySet<string>,
  context: TimelineDisplayContext = {},
): boolean {
  const defaultExpanded = timelineDefaultExpanded(event, context);
  return toggledIds.has(event.id) ? !defaultExpanded : defaultExpanded;
}

export function timelineCanExpand(
  event: DisplayDerivableEvent,
  context: TimelineDisplayContext = {},
): boolean {
  if (isTimelineFinalReply(event)) return true;
  const details = readTimelineDisplay(event, context).details ?? [];
  return details.length > 0;
}

export function toggleTimelineExpandedId(
  toggledIds: ReadonlySet<string>,
  eventId: string,
): Set<string> {
  const next = new Set(toggledIds);
  if (next.has(eventId)) next.delete(eventId);
  else next.add(eventId);
  return next;
}

export function pruneTimelineExpandedIds(
  toggledIds: ReadonlySet<string>,
  events: ReadonlyArray<Pick<AgentTimelineEvent, "id">>,
): Set<string> {
  const currentIds = new Set(events.map((event) => event.id));
  return new Set([...toggledIds].filter((id) => currentIds.has(id)));
}

export function timelineEventLabel(
  event: DisplayDerivableEvent,
  context: TimelineDisplayContext = {},
): string {
  const display = readTimelineDisplay(event, context);
  const label = display.label?.trim();
  if (label) return label;
  const action = display.action?.trim();
  if (!action) return "事件";
  const verb = formatTimelineActionLabel(event.status, action);
  if (display.objectInLabel) {
    const object = display.object?.trim() ?? "";
    if (object) return `${verb} ${object}`;
  }
  return verb;
}

export function timelineGroupKey(event: DisplayDerivableEvent): string | null {
  const display = readTimelineDisplay(event);
  const declaredKey = display.group?.key?.trim();
  if (declaredKey) return `display:${declaredKey}`;
  return null;
}

export function aggregateTimelineStatus(
  events: ReadonlyArray<Pick<AgentTimelineEvent, "status">>,
): AgentTimelineEventStatus {
  if (events.some((e) =>
    e.status === "failed" || e.status === "error" || e.status === "cancelled"
  )) {
    return "failed";
  }
  if (events.some((e) => RUNNING_STATUSES.has(e.status))) return "running";
  return "completed";
}

export function timelineGroupLabel(
  representative: DisplayDerivableEvent,
  count: number,
  status: AgentTimelineEventStatus,
  context: TimelineDisplayContext = {},
): string {
  const display = readTimelineDisplay(representative, context);
  const group = display.group;
  if (group?.key) {
    const unit = group.unit?.trim() || "项";
    const action = display.action?.trim();
    if (action) return `${formatTimelineActionLabel(status, action)} ${count} ${unit}`;
    const label = display.label?.trim() || "事件";
    return `${label} ${count} ${unit}`;
  }

  return `事件 ${count} 项`;
}

export function timelineDisplayIcon(
  event: DisplayDerivableEvent,
  context: TimelineDisplayContext = {},
): string | null {
  return readTimelineDisplay(event, context).icon ?? null;
}

export function timelineDeclaredGroupUnit(
  event: DisplayDerivableEvent,
): TimelineDeclaredGroupUnit | null {
  const group = readTimelineDisplay(event).group;
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

function formatTimelineActionLabel(
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

export function timelineInlinePreview(
  event: DisplayDerivableEvent,
  context: TimelineDisplayContext = {},
): string {
  if (isTimelineFinalReply(event)) return "";
  const display = readTimelineDisplay(event, context);
  const declaredPreview = display.preview?.trim();
  if (declaredPreview) return truncateTimelineText(toSingleLineText(declaredPreview), 180);
  return "";
}

export function createTimelineMarkdownView(
  text: string | null | undefined,
  options: {
    multilineTone?: MarkdownBlockTone;
    singleLineTone?: MarkdownBlockTone;
    forceSingleLine?: boolean;
  } = {},
): TimelineMarkdownView | null {
  const normalized = (text ?? "").replace(/\r\n?/g, "\n").trim();
  if (!normalized) return null;

  const singleLine = options.forceSingleLine || !normalized.includes("\n");
  return {
    content: singleLine ? toSingleLineText(normalized) : normalized,
    tone: singleLine
      ? options.singleLineTone ?? "muted"
      : options.multilineTone ?? "default",
    singleLine,
  };
}

export function truncateTimelineText(
  text: string,
  maxLength = TIMELINE_SUMMARY_MAX_LENGTH,
): string {
  const chars = Array.from(text);
  if (chars.length <= maxLength) return text;
  return `${chars.slice(0, Math.max(0, maxLength)).join("").trimEnd()}...`;
}

function toSingleLineText(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}
