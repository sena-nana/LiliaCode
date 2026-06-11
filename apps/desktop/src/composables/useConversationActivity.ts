import { reactive } from "vue";
import type { ChatRuntimePhase } from "@lilia/contracts";
import type { AgentTimelineEvent } from "@lilia/contracts";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  getRuntimeSnapshot,
  listAgentTimeline,
  onAgentTimeline,
  onDone,
  onTurnStarted,
} from "../services/chat";

export type ConversationActivity = "running" | "requires_action" | "completed" | "error";

interface ConversationActivityState {
  running: boolean;
  completed: boolean;
  error: boolean;
  pendingRequests: string[];
}

const states = reactive<Record<string, ConversationActivityState>>({});
const hydratedTaskIds = new Set<string>();
const timelineRequestIdsByEventId = new Map<string, string>();
let installed = false;
let unlistenAll: UnlistenFn[] = [];

function ensureState(taskId: string): ConversationActivityState {
  const existing = states[taskId];
  if (existing) return existing;
  states[taskId] = {
    running: false,
    completed: false,
    error: false,
    pendingRequests: [],
  };
  return states[taskId];
}

function compactState(taskId: string) {
  const state = states[taskId];
  if (!state) return;
  if (!state.running && !state.completed && !state.error && state.pendingRequests.length === 0) {
    delete states[taskId];
  }
}

export function markConversationRunning(taskId: string) {
  if (!taskId) return;
  const state = ensureState(taskId);
  hydratedTaskIds.add(taskId);
  state.running = true;
  state.completed = false;
  state.error = false;
  state.pendingRequests = [];
}

export function markConversationRequiresAction(taskId: string, requestId: string) {
  if (!taskId || !requestId) return;
  const state = ensureState(taskId);
  hydratedTaskIds.add(taskId);
  if (!state.pendingRequests.includes(requestId)) {
    state.pendingRequests = [...state.pendingRequests, requestId];
  }
  state.completed = false;
  state.error = false;
}

export function clearConversationRequiresAction(taskId: string, requestId?: string | null) {
  if (!taskId) return;
  const state = states[taskId];
  if (!state) return;
  state.pendingRequests = requestId
    ? state.pendingRequests.filter((id) => id !== requestId)
    : [];
  compactState(taskId);
}

export function markConversationCompleted(taskId: string) {
  if (!taskId) return;
  const state = ensureState(taskId);
  hydratedTaskIds.add(taskId);
  state.running = false;
  state.pendingRequests = [];
  if (state.error) return;
  state.completed = true;
}

export function markConversationError(taskId: string) {
  if (!taskId) return;
  const state = ensureState(taskId);
  hydratedTaskIds.add(taskId);
  state.running = false;
  state.completed = false;
  state.pendingRequests = [];
  state.error = true;
}

export function clearConversationActivityNotice(taskId: string) {
  if (!taskId) return;
  const state = states[taskId];
  if (!state || (!state.completed && !state.error)) return;
  state.completed = false;
  state.error = false;
  compactState(taskId);
}

export function resetConversationActivity() {
  for (const taskId of Object.keys(states)) {
    delete states[taskId];
  }
  hydratedTaskIds.clear();
  timelineRequestIdsByEventId.clear();
}

export function conversationActivityForTask(taskId: string): ConversationActivity | null {
  const state = states[taskId];
  if (!state) return null;
  if (state.error) return "error";
  if (state.pendingRequests.length > 0) return "requires_action";
  if (state.running) return "running";
  if (state.completed) return "completed";
  return null;
}

function runtimePhaseToConversationActivity(
  phase: ChatRuntimePhase,
): ConversationActivity | null {
  if (
    phase === "running" ||
    phase === "running_and_queued" ||
    phase === "queued" ||
    phase === "interrupted_pending_finish" ||
    phase === "reset_pending_finish"
  ) {
    return "running";
  }
  if (phase === "abandoned") return "error";
  return null;
}

function readTimelinePayloadString(event: AgentTimelineEvent, key: string): string | null {
  const payload = event.payload;
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const value = (payload as Record<string, unknown>)[key];
  return typeof value === "string" ? value : null;
}

function timelineRequiresActionRequestId(event: AgentTimelineEvent): string | null {
  if (event.status !== "requires_action") return null;
  if (event.kind === "title_update" || event.kind === "plan" || event.kind === "ask_user") {
    return readTimelinePayloadString(event, "requestId") ?? `timeline:${event.id}`;
  }
  const interaction = readTimelinePayloadString(event, "interaction");
  if (
    interaction === "tool_consent" ||
    interaction === "mcp_elicitation" ||
    interaction === "permission_approval"
  ) {
    return readTimelinePayloadString(event, "requestId") ?? `timeline:${event.id}`;
  }
  return null;
}

function timelineRequestId(event: AgentTimelineEvent): string {
  return readTimelinePayloadString(event, "requestId") ??
    timelineRequestIdsByEventId.get(event.id) ??
    `timeline:${event.id}`;
}

function timelineHasError(events: AgentTimelineEvent[]): boolean {
  return events.some((event) => event.kind === "error" || event.status === "error");
}

function timelineRequiresActionRequestIds(events: AgentTimelineEvent[]): string[] {
  const requestIds = new Set<string>();
  for (const event of events) {
    const requestId = timelineRequiresActionRequestId(event);
    if (requestId) requestIds.add(requestId);
  }
  return [...requestIds];
}

export async function hydrateConversationActivities(taskIds: string[]): Promise<void> {
  const pendingTaskIds = Array.from(new Set(taskIds.filter(Boolean))).filter((taskId) =>
    !hydratedTaskIds.has(taskId) && !states[taskId]
  );
  if (pendingTaskIds.length === 0) return;

  const entries = await Promise.all(pendingTaskIds.map(async (taskId) => {
    try {
      const [snapshot, timeline] = await Promise.all([
        getRuntimeSnapshot(taskId),
        listAgentTimeline(taskId),
      ]);
      return { taskId, snapshot, timeline };
    } catch (err) {
      console.error("[conversation-activity] hydrate failed", taskId, err);
      return null;
    }
  }));

  for (const entry of entries) {
    if (!entry) continue;
    const { taskId, snapshot, timeline } = entry;
    hydratedTaskIds.add(taskId);
    if (states[taskId]) continue;
    const runtimeActivity = runtimePhaseToConversationActivity(snapshot.phase);
    if (runtimeActivity === "error") {
      markConversationError(taskId);
      continue;
    }
    if (runtimeActivity === "running") {
      markConversationRunning(taskId);
      continue;
    }
    if (timelineHasError(timeline)) {
      markConversationError(taskId);
      continue;
    }
    for (const requestId of timelineRequiresActionRequestIds(timeline)) {
      markConversationRequiresAction(taskId, requestId);
    }
  }
}

function applyTimelineActivity(event: AgentTimelineEvent) {
  if (event.kind === "error" || event.status === "error") {
    markConversationError(event.taskId);
    return;
  }
  const requiresActionRequestId = timelineRequiresActionRequestId(event);
  if (requiresActionRequestId) {
    timelineRequestIdsByEventId.set(event.id, requiresActionRequestId);
    markConversationRequiresAction(event.taskId, requiresActionRequestId);
    return;
  }
  if (event.status === "requires_action") return;
  const requestId = timelineRequestId(event);
  timelineRequestIdsByEventId.delete(event.id);
  clearConversationRequiresAction(event.taskId, requestId);
}

export async function installConversationActivityBridge(): Promise<() => void> {
  if (installed) return () => {};
  installed = true;
  unlistenAll = await Promise.all([
    onTurnStarted((event) => markConversationRunning(event.taskId)),
    onDone((event) => markConversationCompleted(event.taskId)),
    onAgentTimeline(applyTimelineActivity),
  ]);
  return () => {
    for (const unlisten of unlistenAll) unlisten();
    unlistenAll = [];
    installed = false;
  };
}
