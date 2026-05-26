import type {
  AgentTimelineDisplay,
  AgentTimelineEvent,
  AgentTimelineEventStatus,
  AgentTimelinePayload,
} from "@lilia/contracts";

export type MarkdownBlockTone = "default" | "muted";

export const TIMELINE_SUMMARY_MAX_LENGTH = 220;

export type TimelinePayloadRecord = Record<string, AgentTimelinePayload | undefined>;

export interface TimelineMarkdownView {
  content: string;
  tone: MarkdownBlockTone;
  singleLine: boolean;
}

export interface TimelineDeclaredGroupUnit {
  key: string;
  count: number;
  unit: string | null;
}

const RUNNING_STATUSES = new Set<AgentTimelineEventStatus>([
  "pending",
  "started",
  "running",
  "in_progress",
]);

export function readTimelinePayloadRecord(
  event: Pick<AgentTimelineEvent, "payload">,
): TimelinePayloadRecord {
  return readPayloadRecord(event.payload);
}

export function readTimelineDisplay(
  event: Pick<AgentTimelineEvent, "display">,
): AgentTimelineDisplay {
  return event.display;
}

export function readPayloadRecord(payload: unknown): TimelinePayloadRecord {
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

export function isTimelineAssistantMessage(
  event: Pick<AgentTimelineEvent, "kind" | "payload">,
): boolean {
  if (event.kind !== "message") return false;
  return readTimelinePayloadRecord(event).role === "assistant";
}

/**
 * 时间线里的 `kind === "turn"` 事件全部由 runner 出于会话恢复/调试目的发出
 * （Claude session、Claude status、Claude turn completed、API retry...），对终端
 * 用户都是噪音。统一在 UI 这层过滤；持久化层仍保留它们，session resume 依赖。
 */
export function isHiddenTimelineEvent(
  event: Pick<AgentTimelineEvent, "kind">,
): boolean {
  return event.kind === "turn";
}

/**
 * 「最终回复」= assistant message timeline。流式过程中（status=running）也算，
 * 这样组件树展开/折叠状态在 token 增量到达时不会抖动。
 */
export function isTimelineFinalReply(
  event: Pick<AgentTimelineEvent, "kind" | "payload">,
): boolean {
  return isTimelineAssistantMessage(event);
}

export function isTimelineFinalReplyStreaming(
  event: Pick<AgentTimelineEvent, "kind" | "payload" | "status">,
): boolean {
  return isTimelineAssistantMessage(event) && RUNNING_STATUSES.has(event.status);
}

export function timelineDefaultExpanded(
  event: Pick<AgentTimelineEvent, "kind" | "payload" | "status" | "display">,
): boolean {
  const display = readTimelineDisplay(event);
  if (typeof display?.defaultExpanded === "boolean") return display.defaultExpanded;
  return isTimelineFinalReply(event);
}

export function timelineDefaultCollapsed(
  event: Pick<AgentTimelineEvent, "kind" | "payload" | "status" | "display">,
): boolean {
  return !timelineDefaultExpanded(event);
}

export function isTimelineExpanded(
  event: Pick<AgentTimelineEvent, "id" | "kind" | "payload" | "status" | "display">,
  toggledIds: ReadonlySet<string>,
): boolean {
  const defaultExpanded = timelineDefaultExpanded(event);
  return toggledIds.has(event.id) ? !defaultExpanded : defaultExpanded;
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
  event: Pick<AgentTimelineEvent, "display" | "status">,
): string {
  const display = readTimelineDisplay(event);
  const label = display.label?.trim();
  if (label) return label;
  const action = display.action?.trim();
  if (action) {
    return appendTimelineObjectLabel(
      formatTimelineActionLabel(event.status, action),
      display.object?.trim() ?? "",
    );
  }

  return "事件";
}

export function timelineGroupKey(
  event: Pick<AgentTimelineEvent, "display">,
): string | null {
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
  representative: Pick<AgentTimelineEvent, "display">,
  count: number,
  status: AgentTimelineEventStatus,
): string {
  const display = readTimelineDisplay(representative);
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
  event: Pick<AgentTimelineEvent, "display">,
): string | null {
  return readTimelineDisplay(event).icon ?? null;
}

export function timelineDeclaredGroupUnit(
  event: Pick<AgentTimelineEvent, "display">,
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

function appendTimelineObjectLabel(label: string, objectLabel: string): string {
  return objectLabel ? `${label} ${objectLabel}` : label;
}

export function timelineInlinePreview(
  event: Pick<AgentTimelineEvent, "display" | "kind" | "payload">,
): string {
  if (isTimelineFinalReply(event)) return "";
  const display = readTimelineDisplay(event);
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
