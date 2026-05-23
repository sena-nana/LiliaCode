<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  Brain,
  CheckCircle2,
  CircleDot,
  Code2,
  FilePenLine,
  GitBranch,
  Globe2,
  ListChecks,
  Terminal,
  TriangleAlert,
  UserRound,
  Wrench,
  type LucideIcon,
} from "lucide-vue-next";
import type {
  AgentTimelineEvent,
  AgentTimelineEventKind,
  ChatMessage,
} from "@lilia/contracts";
import ChatBubble from "./ChatBubble.vue";

type PayloadRecord = Record<string, unknown>;
type StreamableMessage = ChatMessage & { streaming?: boolean; queued?: boolean };

type TimelineEntry =
  | {
      type: "message";
      id: string;
      createdAt: number;
      order: number;
      message: StreamableMessage;
    }
  | {
      type: "event";
      id: string;
      createdAt: number;
      order: number;
      event: AgentTimelineEvent;
    };

const props = defineProps<{
  events: AgentTimelineEvent[];
  messages?: StreamableMessage[];
}>();

const toggledIds = ref<Set<string>>(new Set());

const visibleMessages = computed(() =>
  (props.messages ?? []).filter((message) => message.role !== "assistant"),
);

const orderedEntries = computed<TimelineEntry[]>(() =>
  [
    ...visibleMessages.value.map((message): TimelineEntry => ({
      type: "message",
      id: `message:${message.id}`,
      createdAt: message.createdAt,
      order: 0,
      message,
    })),
    ...props.events.map((event): TimelineEntry => ({
      type: "event",
      id: `event:${event.id}`,
      createdAt: event.createdAt,
      order: event.order + 1,
      event,
    })),
  ].sort((a, b) =>
    a.createdAt - b.createdAt || a.order - b.order || a.id.localeCompare(b.id)
  ),
);

watch(
  () => props.events.map((event) => event.id).join("|"),
  (ids) => {
    const current = new Set(ids ? ids.split("|") : []);
    toggledIds.value = new Set([...toggledIds.value].filter((id) => current.has(id)));
  },
);

function payloadRecord(event: AgentTimelineEvent): PayloadRecord {
  return event.payload && typeof event.payload === "object" && !Array.isArray(event.payload)
    ? event.payload as PayloadRecord
    : {};
}

function payloadString(event: AgentTimelineEvent, key: string): string {
  const value = payloadRecord(event)[key];
  return typeof value === "string" ? value : "";
}

function finalText(event: AgentTimelineEvent): string {
  return event.kind === "turn" ? payloadString(event, "finalText") : "";
}

function isFinalReply(event: AgentTimelineEvent): boolean {
  return finalText(event).trim().length > 0 &&
    ["success", "completed", "done"].includes(event.status);
}

function isExpanded(event: AgentTimelineEvent): boolean {
  const toggled = toggledIds.value.has(event.id);
  return isFinalReply(event) ? !toggled : toggled;
}

function toggleEvent(event: AgentTimelineEvent) {
  const next = new Set(toggledIds.value);
  if (next.has(event.id)) next.delete(event.id);
  else next.add(event.id);
  toggledIds.value = next;
}

function eventLabel(event: AgentTimelineEvent): string {
  if (isFinalReply(event)) return "最终回复";
  return event.title.trim() || event.kind;
}

function eventSummary(event: AgentTimelineEvent): string | null {
  if (isFinalReply(event)) return null;
  const summary = event.summary?.trim();
  return summary ? truncate(summary, 220) : null;
}

function eventDetails(event: AgentTimelineEvent): string | null {
  const payload = payloadRecord(event);
  switch (event.kind) {
    case "command":
      return joinLines([
        payloadString(event, "command"),
        payloadString(event, "aggregatedOutput"),
        payloadString(event, "stdout"),
        payloadString(event, "stderr"),
      ]);
    case "file_change":
      return fileChangeSummary(payload.changes);
    case "mcp":
      return joinLines([payloadString(event, "server"), payloadString(event, "tool")]);
    case "web_search":
      return payloadString(event, "query");
    case "tool":
    case "subagent":
      return joinLines([
        payloadString(event, "toolName"),
        payloadString(event, "agentType"),
        payloadString(event, "subagentType"),
        payloadString(event, "taskDescription"),
      ]);
    case "error":
      return joinLines([payloadString(event, "message"), payloadString(event, "error")]);
    default:
      return payloadText(payload);
  }
}

function truncate(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return `${text.slice(0, maxLength).trimEnd()}...`;
}

function joinLines(lines: Array<string | null | undefined>): string | null {
  const text = lines
    .map((line) => line?.trim())
    .filter(Boolean)
    .join("\n");
  return text || null;
}

function payloadText(payload: unknown): string | null {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const candidate = payload as Record<string, unknown>;
  for (const key of ["command", "path", "filePath", "toolName", "query"]) {
    const value = candidate[key];
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  return null;
}

function fileChangeSummary(changes: unknown): string | null {
  if (!Array.isArray(changes)) return null;
  return joinLines(changes.map((change) => {
    if (!change || typeof change !== "object") return String(change);
    const row = change as PayloadRecord;
    const kind = typeof row.kind === "string" ? row.kind : "update";
    const path = typeof row.path === "string" ? row.path : "";
    return path ? `${kind} ${path}` : kind;
  }));
}

function formatPayload(event: AgentTimelineEvent): string | null {
  const payload = payloadRecord(event);
  const { finalText: _finalText, ...rest } = payload;
  if (Object.keys(rest).length === 0) return null;
  try {
    return truncate(JSON.stringify(rest, null, 2), 2000);
  } catch {
    return truncate(String(rest), 2000);
  }
}

function todoItems(event: AgentTimelineEvent): Array<{ text: string; completed: boolean }> {
  const items = payloadRecord(event).items;
  if (!Array.isArray(items)) return [];
  return items
    .map((item) => {
      if (!item || typeof item !== "object") return null;
      const row = item as PayloadRecord;
      const text = typeof row.text === "string" ? row.text : "";
      if (!text.trim()) return null;
      return { text, completed: row.completed === true || row.status === "completed" };
    })
    .filter((item): item is { text: string; completed: boolean } => !!item);
}

function fileChanges(event: AgentTimelineEvent): Array<{ kind: string; path: string }> {
  const changes = payloadRecord(event).changes;
  if (!Array.isArray(changes)) return [];
  return changes
    .map((change) => {
      if (!change || typeof change !== "object") return null;
      const row = change as PayloadRecord;
      const path = typeof row.path === "string" ? row.path : "";
      if (!path.trim()) return null;
      return {
        kind: typeof row.kind === "string" ? row.kind : "update",
        path,
      };
    })
    .filter((change): change is { kind: string; path: string } => !!change);
}

function kindLabel(kind: AgentTimelineEventKind): string {
  const labels: Record<AgentTimelineEventKind, string> = {
    reasoning: "思考",
    plan: "计划",
    todo_list: "Todo",
    tool: "工具",
    command: "命令",
    subagent: "子代理",
    file_change: "修改",
    mcp: "MCP",
    web_search: "搜索",
    error: "错误",
    turn: "回合",
  };
  return labels[kind] ?? kind;
}

function statusLabel(status: string): string {
  const labels: Record<string, string> = {
    pending: "等待",
    started: "开始",
    running: "运行中",
    in_progress: "运行中",
    completed: "完成",
    done: "完成",
    success: "完成",
    failed: "失败",
    error: "失败",
    cancelled: "取消",
    skipped: "跳过",
    info: "信息",
    requires_action: "待处理",
  };
  return labels[status] ?? status;
}

function statusClass(status: string): Record<string, boolean> {
  return {
    "is-running": ["pending", "started", "running", "in_progress"].includes(status),
    "is-completed": ["completed", "done", "success"].includes(status),
    "is-error": ["failed", "error"].includes(status),
  };
}

function kindIcon(kind: AgentTimelineEventKind): LucideIcon {
  const icons: Record<AgentTimelineEventKind, LucideIcon> = {
    reasoning: Brain,
    plan: ListChecks,
    todo_list: CheckCircle2,
    tool: Wrench,
    command: Terminal,
    subagent: GitBranch,
    file_change: FilePenLine,
    mcp: Code2,
    web_search: Globe2,
    error: TriangleAlert,
    turn: CircleDot,
  };
  return icons[kind] ?? CircleDot;
}
</script>

<template>
  <section
    v-if="orderedEntries.length"
    class="agent-timeline"
    aria-label="Agent 工作过程"
  >
    <ol class="agent-timeline__list">
      <template v-for="entry in orderedEntries" :key="entry.id">
        <li
          v-if="entry.type === 'message'"
          class="agent-timeline__message-row"
          :class="[
            `agent-timeline__message-row--${entry.message.role}`,
            { 'is-queued': entry.message.queued },
          ]"
        >
          <span class="agent-timeline__message-icon" aria-hidden="true">
            <UserRound :size="14" />
          </span>
          <ChatBubble :message="entry.message" />
        </li>
        <li
          v-else
          class="agent-timeline__item"
          :class="[
            `agent-timeline--${entry.event.kind}`,
            `agent-timeline__item--${entry.event.status}`,
            statusClass(entry.event.status),
            { 'is-expanded': isExpanded(entry.event), 'is-final-reply': isFinalReply(entry.event) },
          ]"
        >
          <span class="agent-timeline__icon" aria-hidden="true">
            <component :is="kindIcon(entry.event.kind)" :size="14" />
          </span>
          <article
            class="agent-timeline__event"
            :aria-labelledby="`agent-timeline-title-${entry.event.id}`"
          >
            <div class="agent-timeline__body">
              <header class="agent-timeline__head">
                <button
                  type="button"
                  class="agent-timeline__title"
                  :aria-expanded="isExpanded(entry.event)"
                  :aria-controls="`agent-timeline-details-${entry.event.id}`"
                  @click="toggleEvent(entry.event)"
                >
                  <span :id="`agent-timeline-title-${entry.event.id}`">
                    {{ eventLabel(entry.event) }}
                  </span>
                </button>
                <div class="agent-timeline__meta" aria-label="事件分类和状态">
                  <span class="agent-timeline__badge">{{ kindLabel(entry.event.kind) }}</span>
                  <span class="agent-timeline__badge">{{ statusLabel(entry.event.status) }}</span>
                </div>
              </header>

              <p v-if="eventSummary(entry.event)" class="agent-timeline__summary">
                {{ eventSummary(entry.event) }}
              </p>

              <div
                v-if="isExpanded(entry.event)"
                :id="`agent-timeline-details-${entry.event.id}`"
                class="agent-timeline__details"
            >
              <div v-if="isFinalReply(entry.event)" class="agent-timeline__final">
                {{ finalText(entry.event) }}
              </div>

              <ul
                v-else-if="entry.event.kind === 'todo_list' && todoItems(entry.event).length"
                class="agent-timeline__todo-list"
              >
                <li
                  v-for="item in todoItems(entry.event)"
                  :key="item.text"
                  :class="{ 'is-done': item.completed }"
                >
                  <span aria-hidden="true">{{ item.completed ? "✓" : "•" }}</span>
                  <span>{{ item.text }}</span>
                </li>
              </ul>

              <ul
                v-else-if="entry.event.kind === 'file_change' && fileChanges(entry.event).length"
                class="agent-timeline__file-list"
              >
                <li v-for="change in fileChanges(entry.event)" :key="`${change.kind}:${change.path}`">
                  <span>{{ change.kind }}</span>
                  <code>{{ change.path }}</code>
                </li>
              </ul>

              <p v-else-if="eventDetails(entry.event)" class="agent-timeline__details-text">
                {{ eventDetails(entry.event) }}
              </p>

              <pre
                v-if="formatPayload(entry.event)"
                class="agent-timeline__payload"
              ><code>{{ formatPayload(entry.event) }}</code></pre>
              </div>
            </div>
          </article>
        </li>
      </template>
    </ol>
  </section>
</template>
