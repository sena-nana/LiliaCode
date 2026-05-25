import type {
  AgentTimelineEvent,
  AgentTimelineEventStatus,
  AgentTimelinePayload,
} from "@lilia/contracts";

export type MarkdownBlockTone = "default" | "muted";

export const TIMELINE_SUMMARY_MAX_LENGTH = 220;
export const TIMELINE_PAYLOAD_MAX_LENGTH = 2000;

export type TimelinePayloadRecord = Record<string, AgentTimelinePayload | undefined>;

export interface TimelineMarkdownView {
  content: string;
  tone: MarkdownBlockTone;
  singleLine: boolean;
}

export interface TimelineTodoItem {
  text: string;
  completed: boolean;
}

export interface TimelineFileChange {
  kind: string;
  path: string;
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

export function readPayloadRecord(payload: unknown): TimelinePayloadRecord {
  return payload && typeof payload === "object" && !Array.isArray(payload)
    ? payload as TimelinePayloadRecord
    : {};
}

export function readTimelinePayloadString(
  event: Pick<AgentTimelineEvent, "payload">,
  key: string,
): string {
  const value = readTimelinePayloadRecord(event)[key];
  return typeof value === "string" ? value : "";
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
 * 「最终回复」= assistant message timeline。流式过程中（status=running）也算，
 * 这样组件树展开/折叠状态在 token 增量到达时不会抖动。
 */
export function isTimelineFinalReply(
  event: Pick<AgentTimelineEvent, "kind" | "payload" | "status">,
): boolean {
  return isTimelineAssistantMessage(event);
}

export function isTimelineFinalReplyStreaming(
  event: Pick<AgentTimelineEvent, "kind" | "payload" | "status">,
): boolean {
  return isTimelineAssistantMessage(event) && RUNNING_STATUSES.has(event.status);
}

export function timelineDefaultExpanded(
  event: Pick<AgentTimelineEvent, "kind" | "payload" | "status">,
): boolean {
  return isTimelineFinalReply(event);
}

export function timelineDefaultCollapsed(
  event: Pick<AgentTimelineEvent, "kind" | "payload" | "status">,
): boolean {
  return !timelineDefaultExpanded(event);
}

export function isTimelineExpanded(
  event: Pick<AgentTimelineEvent, "id" | "kind" | "payload" | "status">,
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
  event: Pick<AgentTimelineEvent, "kind" | "payload" | "status" | "title">,
): string {
  return event.title.trim() || event.kind;
}

export function timelineEventSummary(
  event: Pick<AgentTimelineEvent, "kind" | "payload" | "status" | "summary">,
): TimelineMarkdownView | null {
  if (isTimelineFinalReply(event)) return null;
  const summary = event.summary?.trim();
  if (!summary) return null;
  return {
    content: truncateTimelineText(toSingleLineText(summary), TIMELINE_SUMMARY_MAX_LENGTH),
    tone: "muted",
    singleLine: true,
  };
}

export function timelineInlinePreview(
  event: Pick<AgentTimelineEvent, "kind" | "payload" | "status" | "summary" | "title">,
): string {
  if (isTimelineFinalReply(event)) return "";
  const payload = readTimelinePayloadRecord(event);
  const summary = event.summary?.trim();

  const preview = (() => {
    switch (event.kind) {
      case "command":
        return summary || readFirstPayloadString(payload, [
          "command",
          "cmd",
          "shellCommand",
          "script",
          "argv",
        ]);
      case "file_change":
        return summary || timelineFileChangeInlinePreview(payload);
      case "mcp": {
        const target = [
          readFirstPayloadString(payload, ["server", "serverName", "mcpServer"]),
          readFirstPayloadString(payload, ["tool", "toolName", "name"]),
        ].filter(Boolean).join("/");
        return summary || target;
      }
      case "web_search":
        return summary || readFirstPayloadString(payload, ["query", "searchQuery", "q", "url"]);
      case "tool":
        return summary || readFirstPayloadString(payload, [
          "toolName",
          "name",
          "tool",
          "function",
        ]);
      case "subagent": {
        const agentName = readFirstPayloadString(payload, [
          "agentType",
          "subagentType",
          "agentName",
          "name",
          "type",
        ]);
        const task = readFirstPayloadString(payload, [
          "taskDescription",
          "description",
          "prompt",
          "task",
        ]);
        return summary || [agentName, task].filter(Boolean).join(": ");
      }
      case "todo_list":
        return summary || timelineTodoInlinePreview(payload);
      case "message":
        return "";
      case "plan":
      case "reasoning":
      case "turn":
      case "error":
      default:
        return summary || readFirstPayloadString(payload, [
          "summary",
          "message",
          "error",
          "text",
          "content",
          "result",
          "details",
        ]);
    }
  })();

  return preview ? truncateTimelineText(toSingleLineText(preview), 180) : "";
}

export function timelineEventDetails(
  event: Pick<AgentTimelineEvent, "kind" | "payload" | "status">,
): TimelineMarkdownView | null {
  if (isTimelineFinalReply(event)) {
    return createTimelineMarkdownView(timelineFinalText(event), {
      multilineTone: "default",
      singleLineTone: "default",
    });
  }

  return createTimelineMarkdownView(timelineEventDetailsText(event), {
    multilineTone: "default",
    singleLineTone: "muted",
  });
}

export function timelineEventDetailsText(
  event: Pick<AgentTimelineEvent, "kind" | "payload">,
): string | null {
  const payload = readTimelinePayloadRecord(event);

  switch (event.kind) {
    case "command":
      return joinTimelineLines([
        readTimelinePayloadString(event, "command"),
        readTimelinePayloadString(event, "aggregatedOutput"),
        readTimelinePayloadString(event, "stdout"),
        readTimelinePayloadString(event, "stderr"),
      ]);
    case "file_change":
      return timelineFileChangeSummary(payload.changes);
    case "mcp":
      return joinTimelineLines([
        readTimelinePayloadString(event, "server"),
        readTimelinePayloadString(event, "tool"),
      ]);
    case "web_search":
      return readTimelinePayloadString(event, "query");
    case "tool":
    case "subagent":
      return joinTimelineLines([
        readTimelinePayloadString(event, "toolName"),
        readTimelinePayloadString(event, "agentType"),
        readTimelinePayloadString(event, "subagentType"),
        readTimelinePayloadString(event, "taskDescription"),
      ]);
    case "error":
      return joinTimelineLines([
        readTimelinePayloadString(event, "message"),
        readTimelinePayloadString(event, "error"),
      ]);
    default:
      return timelinePayloadPreviewText(payload);
  }
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

export function joinTimelineLines(lines: Array<string | null | undefined>): string | null {
  const text = lines
    .map((line) => line?.trim())
    .filter((line): line is string => Boolean(line))
    .join("\n");
  return text || null;
}

export function formatTimelinePayload(
  event: Pick<AgentTimelineEvent, "payload">,
  maxLength = TIMELINE_PAYLOAD_MAX_LENGTH,
): string | null {
  const payload = readTimelinePayloadRecord(event);
  if (Object.keys(payload).length === 0) return null;

  try {
    return truncateTimelineText(JSON.stringify(payload, null, 2), maxLength);
  } catch {
    return truncateTimelineText(String(payload), maxLength);
  }
}

export function timelineTodoItems(
  event: Pick<AgentTimelineEvent, "payload">,
): TimelineTodoItem[] {
  const items = readTimelinePayloadRecord(event).items;
  if (!Array.isArray(items)) return [];

  return items
    .map((item) => {
      if (!item || typeof item !== "object" || Array.isArray(item)) return null;
      const row = item as TimelinePayloadRecord;
      const text = typeof row.text === "string" ? row.text : "";
      if (!text.trim()) return null;
      return {
        text,
        completed: row.completed === true || row.status === "completed",
      };
    })
    .filter((item): item is TimelineTodoItem => item !== null);
}

export function timelineFileChanges(
  event: Pick<AgentTimelineEvent, "payload">,
): TimelineFileChange[] {
  const changes = readTimelinePayloadRecord(event).changes;
  if (!Array.isArray(changes)) return [];

  return changes
    .map((change) => {
      if (!change || typeof change !== "object" || Array.isArray(change)) return null;
      const row = change as TimelinePayloadRecord;
      const path = typeof row.path === "string" ? row.path : "";
      if (!path.trim()) return null;
      return {
        kind: typeof row.kind === "string" ? row.kind : "update",
        path,
      };
    })
    .filter((change): change is TimelineFileChange => change !== null);
}

function toSingleLineText(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

function timelinePayloadPreviewText(payload: TimelinePayloadRecord): string | null {
  for (const key of ["command", "path", "filePath", "toolName", "query"]) {
    const value = payload[key];
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  return null;
}

function timelineFileChangeSummary(changes: AgentTimelinePayload | undefined): string | null {
  if (!Array.isArray(changes)) return null;
  return joinTimelineLines(changes.map((change) => {
    if (!change || typeof change !== "object" || Array.isArray(change)) {
      return String(change);
    }
    const row = change as TimelinePayloadRecord;
    const kind = typeof row.kind === "string" ? row.kind : "update";
    const path = typeof row.path === "string" ? row.path : "";
    return path ? `${kind} ${path}` : kind;
  }));
}

function timelineFileChangeInlinePreview(payload: TimelinePayloadRecord): string {
  const changes = payload.changes;
  if (Array.isArray(changes) && changes.length > 0) {
    const first = readPayloadRecord(changes[0]);
    const path = readFirstPayloadString(first, [
      "path",
      "filePath",
      "relativePath",
      "targetPath",
      "name",
    ]);
    const kind = readFirstPayloadString(first, ["kind", "operation", "type", "status"]) || "update";
    const suffix = changes.length > 1 ? ` 等 ${changes.length} 个文件` : "";
    return path ? `${kind} ${path}${suffix}` : `${kind}${suffix}`;
  }

  const path = readFirstPayloadString(payload, [
    "path",
    "filePath",
    "relativePath",
    "targetPath",
    "name",
  ]);
  if (!path) return "";
  const kind = readFirstPayloadString(payload, ["kind", "operation", "type", "status"]) || "update";
  return `${kind} ${path}`;
}

function timelineTodoInlinePreview(payload: TimelinePayloadRecord): string {
  const rawItems = [
    payload.items,
    payload.todos,
    readPayloadRecord(payload.input).items,
    readPayloadRecord(payload.input).todos,
  ].find((value) => Array.isArray(value));

  if (!Array.isArray(rawItems) || rawItems.length === 0) return "";

  let completed = 0;
  let firstOpen = "";
  for (const item of rawItems) {
    const row = readPayloadRecord(item);
    const text = typeof item === "string"
      ? item.trim()
      : readFirstPayloadString(row, ["text", "content", "title", "description"]);
    const status = typeof row.status === "string" ? row.status.toLowerCase() : "";
    const done = row.completed === true || row.done === true || status === "completed";
    if (done) completed += 1;
    if (!done && !firstOpen) firstOpen = text;
  }

  return `${completed}/${rawItems.length} 已完成${firstOpen ? ` · ${firstOpen}` : ""}`;
}

function readFirstPayloadString(payload: TimelinePayloadRecord, keys: string[]): string {
  for (const key of keys) {
    const value = payload[key];
    const text = stringifyPayloadInline(value);
    if (text) return text;
  }
  return "";
}

function stringifyPayloadInline(value: unknown): string {
  if (typeof value === "string") return value.trim();
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) {
    return value.map((item) => stringifyPayloadInline(item)).filter(Boolean).join(" ").trim();
  }
  if (value && typeof value === "object") {
    const row = value as TimelinePayloadRecord;
    return readFirstPayloadString(row, [
      "text",
      "title",
      "summary",
      "content",
      "message",
      "name",
      "path",
    ]);
  }
  return "";
}
