import {
  aggregateTimelineStatus,
  deriveTimelineDisplay,
  isHiddenTimelineEvent,
  isTimelineErrorReply,
  isTimelineFinalReply,
  isTimelineFinalReplyStreaming,
  isTimelineInterruptEvent,
  isTimelineMessageEvent,
  isTimelineProcessAnchor,
  isTimelineUserMessage,
  readTimelineEventPayloadRecord,
  readTimelineFinalText,
  timelineDeclaredGroupUnitFromDisplay,
  timelineEventLabelFromDisplay,
  timelineGroupLabelFromDisplay,
  type AgentTimelineDisplay,
  type AgentTimelineEvent,
  type AgentTimelineEventStatus,
  type AgentTimelinePayload,
  type TimelineDeclaredGroupUnit,
} from "@lilia/contracts";
import { measurePerfSync } from "@lilia/ui/diagnostics";

export {
  aggregateTimelineStatus,
  isHiddenTimelineEvent,
  isTimelineErrorReply,
  isTimelineFinalReply,
  isTimelineFinalReplyStreaming,
  isTimelineInterruptEvent,
  isTimelineMessageEvent,
  isTimelineProcessAnchor,
  isTimelineUserMessage,
};

export type MarkdownBlockTone = "default" | "muted";

const TIMELINE_SUMMARY_MAX_LENGTH = 220;
const timelinePayloadRecordCache = new WeakMap<object, TimelinePayloadRecord>();
const timelineDisplayCache = new WeakMap<object, Map<string, AgentTimelineDisplay>>();
const timelineFinalTextCache = new WeakMap<object, string>();

export type TimelinePayloadRecord = Record<string, AgentTimelinePayload | undefined>;

export interface TimelineMarkdownView {
  content: string;
  tone: MarkdownBlockTone;
  singleLine: boolean;
}

export interface TimelineDisplayContext {
  projectCwd?: string | null;
  activePlanApprovalTurnId?: string | null;
}

type DisplayDerivableEvent = Pick<
  AgentTimelineEvent,
  "kind" | "status" | "title" | "summary" | "payload"
> & Partial<Pick<AgentTimelineEvent, "turnId">>;

export function readTimelinePayloadRecord(
  event: Pick<AgentTimelineEvent, "payload">,
): TimelinePayloadRecord {
  const cached = timelinePayloadRecordCache.get(event);
  if (cached) return cached;
  const record = readTimelineEventPayloadRecord(event);
  timelinePayloadRecordCache.set(event, record);
  return record;
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
  const cacheKey = context.projectCwd ?? "";
  let cachedByContext = timelineDisplayCache.get(event);
  const cached = cachedByContext?.get(cacheKey);
  if (cached) return cached;
  const display = measurePerfSync(
    "timeline.display.derive",
    () => deriveTimelineDisplay({
      kind: event.kind,
      status: event.status,
      title: event.title,
      summary: event.summary,
      payload: event.payload,
      projectCwd: context.projectCwd,
    }),
    { detail: `${event.kind}:${event.status}` },
  );
  if (!cachedByContext) {
    cachedByContext = new Map<string, AgentTimelineDisplay>();
    timelineDisplayCache.set(event, cachedByContext);
  }
  cachedByContext.set(cacheKey, display);
  return display;
}

export function timelineFinalText(
  event: Pick<AgentTimelineEvent, "kind" | "status" | "title" | "summary" | "payload">,
): string {
  const cached = timelineFinalTextCache.get(event);
  if (cached !== undefined) return cached;
  const text = readTimelineFinalText(event);
  timelineFinalTextCache.set(event, text);
  return text;
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
  return timelineEventLabelFromDisplay(display, event.status);
}

export function timelineGroupKey(event: DisplayDerivableEvent): string | null {
  const display = readTimelineDisplay(event);
  const declaredKey = display.group?.key?.trim();
  if (declaredKey) return `display:${declaredKey}`;
  return null;
}

export function timelineGroupLabel(
  representative: DisplayDerivableEvent,
  count: number,
  status: AgentTimelineEventStatus,
  context: TimelineDisplayContext = {},
): string {
  const display = readTimelineDisplay(representative, context);
  return timelineGroupLabelFromDisplay(display, count, status);
}

export function timelineDisplayIcon(
  event: DisplayDerivableEvent,
  context: TimelineDisplayContext = {},
): string | null {
  return readTimelineDisplay(event, context).icon ?? null;
}

export function timelineStatusClass(status: AgentTimelineEventStatus): string {
  return `is-status-${status.replace(/_/g, "-")}`;
}

export function timelineKindClass(prefix: string, kind: string): string {
  return `${prefix}${kind.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
}

export function timelineDeclaredGroupUnit(
  event: DisplayDerivableEvent,
): TimelineDeclaredGroupUnit | null {
  return timelineDeclaredGroupUnitFromDisplay(readTimelineDisplay(event));
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
