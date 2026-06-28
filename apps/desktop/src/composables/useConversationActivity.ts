import { reactive } from "vue";
import {
  agentTimelineActionDescriptor,
  agentTimelineActionRequestIds,
  agentTimelineEventRequestId,
  timelineEventScopedKey,
  type AgentTimelineEvent,
  type ChatRuntimePhase,
} from "@lilia/contracts";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  getRuntimeSnapshot,
  listAgentTimeline,
  onAgentTimelineEvents,
  onDone,
  onTurnStarted,
} from "../services/chat";
import { installUnlistenFns, runUnlistenFns } from "../utils/eventListeners";
import { measurePerfAsync } from "../utils/perf";

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
const pendingHydrationQueue = new Map<string, { priority: number; seq: number }>();
const pendingHydrationTaskIds = new Set<string>();
const activeHydrationTaskIds = new Set<string>();
const hydrationWaiters = new Set<{ taskIds: Set<string>; resolve: () => void }>();
const HYDRATION_CONCURRENCY = 3;
let installed = false;
let unlistenAll: UnlistenFn[] = [];
let hydrationSeq = 0;
let activityGeneration = 0;

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

export function clearConversationRunning(taskId: string) {
  if (!taskId) return;
  const state = states[taskId];
  if (!state) return;
  state.running = false;
  compactState(taskId);
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
  activityGeneration += 1;
  for (const taskId of Object.keys(states)) {
    delete states[taskId];
  }
  hydratedTaskIds.clear();
  timelineRequestIdsByEventId.clear();
  pendingHydrationQueue.clear();
  pendingHydrationTaskIds.clear();
  activeHydrationTaskIds.clear();
  for (const waiter of hydrationWaiters) {
    waiter.resolve();
  }
  hydrationWaiters.clear();
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

function isDraftConversationId(taskId: string): boolean {
  return taskId.startsWith("t-draft-") || taskId.startsWith("o-draft-");
}

function timelineRequestId(event: AgentTimelineEvent): string {
  return agentTimelineEventRequestId(event, timelineRequestIdsByEventId.get(timelineEventScopedKey(event)));
}

function clearTimelineRequestActivity(event: AgentTimelineEvent): void {
  const requestId = timelineRequestId(event);
  timelineRequestIdsByEventId.delete(timelineEventScopedKey(event));
  clearConversationRequiresAction(event.taskId, requestId);
}

function timelineHasError(events: AgentTimelineEvent[]): boolean {
  return events.some((event) => event.kind === "error" || event.status === "error");
}

export interface HydrateConversationActivityOptions {
  priorityTaskIds?: string[];
}

function shouldSkipHydration(taskId: string): boolean {
  return !taskId ||
    isDraftConversationId(taskId) ||
    hydratedTaskIds.has(taskId) ||
    !!states[taskId];
}

function collectPendingTaskIds(taskIds: string[]): string[] {
  return Array.from(new Set(taskIds.filter(Boolean))).filter((taskId) => !shouldSkipHydration(taskId));
}

function resolveHydrationWaiters() {
  for (const waiter of [...hydrationWaiters]) {
    const done = [...waiter.taskIds].every((taskId) =>
      shouldSkipHydration(taskId) ||
      (!pendingHydrationTaskIds.has(taskId) && !activeHydrationTaskIds.has(taskId))
    );
    if (!done) continue;
    hydrationWaiters.delete(waiter);
    waiter.resolve();
  }
}

function nextQueuedHydrationTaskId(): string | null {
  let nextTaskId: string | null = null;
  let nextPriority = Number.POSITIVE_INFINITY;
  let nextSeq = Number.POSITIVE_INFINITY;
  for (const [taskId, entry] of pendingHydrationQueue) {
    if (
      entry.priority < nextPriority ||
      (entry.priority === nextPriority && entry.seq < nextSeq)
    ) {
      nextTaskId = taskId;
      nextPriority = entry.priority;
      nextSeq = entry.seq;
    }
  }
  return nextTaskId;
}

async function hydrateSingleConversationActivity(taskId: string, generation: number) {
  try {
    const [snapshot, timeline] = await Promise.all([
      measurePerfAsync(
        "conversation.activity.runtime",
        () => getRuntimeSnapshot(taskId),
        { detail: taskId },
      ),
      measurePerfAsync(
        "conversation.activity.timeline",
        () => listAgentTimeline(taskId),
        { detail: taskId },
      ),
    ]);
    if (generation !== activityGeneration) return;
    hydratedTaskIds.add(taskId);
    if (states[taskId]) return;
    const runtimeActivity = runtimePhaseToConversationActivity(snapshot.phase);
    if (runtimeActivity === "error") {
      markConversationError(taskId);
      return;
    }
    if (runtimeActivity === "running") {
      markConversationRunning(taskId);
      return;
    }
    if (timelineHasError(timeline)) {
      markConversationError(taskId);
      return;
    }
    for (const requestId of agentTimelineActionRequestIds(timeline)) {
      markConversationRequiresAction(taskId, requestId);
    }
  } catch (err) {
    console.error("[conversation-activity] hydrate failed", taskId, err);
  }
}

function pumpConversationActivityHydrationQueue() {
  while (activeHydrationTaskIds.size < HYDRATION_CONCURRENCY) {
    const taskId = nextQueuedHydrationTaskId();
    if (!taskId) break;
    pendingHydrationQueue.delete(taskId);
    pendingHydrationTaskIds.delete(taskId);
    if (shouldSkipHydration(taskId) || activeHydrationTaskIds.has(taskId)) {
      continue;
    }
    activeHydrationTaskIds.add(taskId);
    const generation = activityGeneration;
    void hydrateSingleConversationActivity(taskId, generation).finally(() => {
      activeHydrationTaskIds.delete(taskId);
      resolveHydrationWaiters();
      pumpConversationActivityHydrationQueue();
    });
  }
  resolveHydrationWaiters();
}

export async function hydrateConversationActivities(
  taskIds: string[],
  options: HydrateConversationActivityOptions = {},
): Promise<void> {
  const pendingTaskIds = collectPendingTaskIds(taskIds);
  if (pendingTaskIds.length === 0) return;
  const requestedTaskIds = new Set(pendingTaskIds);
  const priorityOrder = new Map<string, number>();
  for (const [index, taskId] of Array.from(new Set(options.priorityTaskIds ?? [])).entries()) {
    if (requestedTaskIds.has(taskId)) {
      priorityOrder.set(taskId, index);
    }
  }
  for (const [index, taskId] of pendingTaskIds.entries()) {
    const priority = priorityOrder.get(taskId) ?? ((options.priorityTaskIds?.length ?? 0) + index);
    const existing = pendingHydrationQueue.get(taskId);
    if (!existing || priority < existing.priority) {
      pendingHydrationQueue.set(taskId, { priority, seq: ++hydrationSeq });
    }
    pendingHydrationTaskIds.add(taskId);
  }
  pumpConversationActivityHydrationQueue();
  await new Promise<void>((resolve) => {
    const waiter = { taskIds: requestedTaskIds, resolve };
    hydrationWaiters.add(waiter);
    resolveHydrationWaiters();
  });
}

function applyTimelineActivity(event: AgentTimelineEvent) {
  if (event.kind === "error" || event.status === "error") {
    markConversationError(event.taskId);
    return;
  }
  const action = agentTimelineActionDescriptor(event);
  if (action) {
    const memoryKey = timelineEventScopedKey(event);
    const previousRequestId = timelineRequestIdsByEventId.get(memoryKey);
    if (previousRequestId && previousRequestId !== action.requestId) {
      clearConversationRequiresAction(event.taskId, previousRequestId);
    }
    timelineRequestIdsByEventId.set(memoryKey, action.requestId);
    markConversationRequiresAction(event.taskId, action.requestId);
    return;
  }
  clearTimelineRequestActivity(event);
  if (event.status === "requires_action") return;
}

export async function installConversationActivityBridge(): Promise<() => void> {
  if (installed) return () => {};
  installed = true;
  try {
    unlistenAll = await installUnlistenFns([
      () => onTurnStarted((event) => markConversationRunning(event.taskId)),
      () => onDone((event) => markConversationCompleted(event.taskId)),
      () => onAgentTimelineEvents((events) => {
        for (const event of events) applyTimelineActivity(event);
      }),
    ]);
  } catch (err) {
    unlistenAll = [];
    installed = false;
    throw err;
  }
  return () => {
    runUnlistenFns(unlistenAll.splice(0).reverse());
    unlistenAll = [];
    installed = false;
  };
}

