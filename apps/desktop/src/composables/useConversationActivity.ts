import { reactive } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  onAgentInteractionRequest,
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
  state.running = true;
  state.completed = false;
  state.error = false;
  state.pendingRequests = [];
}

export function markConversationRequiresAction(taskId: string, requestId: string) {
  if (!taskId || !requestId) return;
  const state = ensureState(taskId);
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
  state.running = false;
  state.pendingRequests = [];
  if (state.error) return;
  state.completed = true;
}

export function markConversationError(taskId: string) {
  if (!taskId) return;
  const state = ensureState(taskId);
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

export async function installConversationActivityBridge(): Promise<() => void> {
  if (installed) return () => {};
  installed = true;
  unlistenAll = await Promise.all([
    onTurnStarted((event) => markConversationRunning(event.taskId)),
    onDone((event) => markConversationCompleted(event.taskId)),
    onAgentTimeline((event) => {
      if (event.kind === "error" || event.status === "error") {
        markConversationError(event.taskId);
      }
    }),
    onAgentInteractionRequest((event) =>
      markConversationRequiresAction(event.taskId, event.requestId)
    ),
  ]);
  return () => {
    for (const unlisten of unlistenAll) unlisten();
    unlistenAll = [];
    installed = false;
  };
}
