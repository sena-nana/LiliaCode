<script setup lang="ts">
/**
 * Task 详情 = 聊天面板。承载两种入口：
 *   /projects/:projectId/tasks/:taskId —— 绑定到某个项目的任务对话
 *   /chats/:taskId                     —— 不绑定项目的收集箱/草稿对话
 *
 * 用户输入、Agent 过程、最终回复和错误提示统一走 timeline 呈现；
 * 不再通过 transcript 维护第二套可见消息流。
 * projectId 缺省时进入 orphan 模式：cwd 退化到用户家目录；首次发送把草稿 promote 到收集箱。
 */

import { computed, onMounted, onUnmounted, ref, shallowRef, watch } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { homeDir } from "@tauri-apps/api/path";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  getOrphanConversation,
  isDraftOrphan,
  isDraftTask,
  promoteDraftOrphan,
  promoteDraftTask,
} from "../services/tasksStore";
import { getProject } from "../services/projectsStore";
import ChatTranscript from "../components/chat/ChatTranscript.vue";
import ChatComposer from "../components/chat/ChatComposer.vue";
import ChatSidebarHost from "../components/chat/ChatSidebarHost.vue";
import TodoFloat from "../components/todo/TodoFloat.vue";
import {
  resolveAskUserById,
  useAskUserForTask,
  usePendingAsksForTask,
} from "../composables/useAskUser";
import {
  usePendingAgentActionsForTask,
  type PendingAgentActionResolution,
} from "../composables/usePendingAgentActions";
import {
  respondConsent,
  usePendingToolConsentsForTask,
  useToolConsentForTask,
} from "../composables/useToolConsentBridge";
import {
  createTodo,
  listTodos,
  updateTodo,
  type TaskTodo,
  type TaskTodoPriority,
} from "../services/todos";
import {
  getComposerState,
  listAgentTimeline,
  listModels,
  onAgentTimeline,
  onDone,
  onTurnStarted,
  describeAttachments,
  interruptTurn,
  pickAttachmentFiles,
  sendMessage,
  setComposerState,
  type ToolConsentDecision,
  type ToolConsentUpdatedInput,
} from "../services/chat";
import {
  loadAgentInteractionSettings,
  useAgentInteractionSettings,
} from "../composables/useAgentInteractionSettings";
import { onDebugTimelineEvent } from "../composables/useDebugTimelineEvents";
import { registerDebugChatSidebarPanel } from "../composables/useDebugChatSidebarPanel";
import { isAgentTimelineToolWindowKind } from "@lilia/contracts";
import type {
  AskUserResult,
  AgentTimelineEvent,
  AgentTimelinePayload,
  ChatAttachment,
  ChatComposerState,
  ChatModelOption,
} from "@lilia/contracts";

const props = defineProps<{ projectId?: string; taskId: string }>();

const project = computed(() =>
  props.projectId ? getProject(props.projectId) : undefined,
);
const orphan = computed(() =>
  props.projectId ? undefined : getOrphanConversation(props.taskId),
);

/** 路由是否已找到承载对话的项目或孤儿；都没有 → 显示未找到。 */
const hasContext = computed(() => !!project.value || !!orphan.value);

/** 空状态标题：绑了项目就用项目名补全。 */
const emptyHeadline = computed(() =>
  project.value
    ? `要在 ${project.value.name} 中构建什么？`
    : "今天想做什么？",
);

const persistedTimelineEvents = shallowRef<AgentTimelineEvent[]>([]);
const overlayTimelineEvents = shallowRef<AgentTimelineEvent[]>([]);
const timelineEvents = computed(() =>
  mergeTimelineEvents(persistedTimelineEvents.value, overlayTimelineEvents.value),
);
const composer = ref<ChatComposerState>({
  taskId: props.taskId,
  backend: "claude",
  model: "claude-sonnet-4-6",
  planMode: false,
  permission: "ask",
});
const models = ref<ChatModelOption[]>([]);
const isTurnRunning = ref(false);
const chatPageRef = ref<HTMLElement | null>(null);
const attachments = ref<ChatAttachment[]>([]);
const userSendScrollKey = ref(0);
const pendingAskUser = useAskUserForTask(() => props.taskId);
const pendingAskUsers = usePendingAsksForTask(() => props.taskId);
const pendingToolConsent = useToolConsentForTask(() => props.taskId);
const pendingToolConsents = usePendingToolConsentsForTask(() => props.taskId);
const runtimePendingAgentActions = usePendingAgentActionsForTask(
  pendingAskUsers,
  pendingToolConsents,
);
const agentInteractionSettings = useAgentInteractionSettings();
const nonInterruptMode = agentInteractionSettings.nonInterruptMode;
const pendingAgentActions = computed(() =>
  nonInterruptMode.value ? runtimePendingAgentActions.value : [],
);

const pendingPlanApproval = computed(() => {
  const ask = nonInterruptMode.value
    ? pendingAskUsers.value.find((item) => item.spec.intent === "plan_approval") ?? null
    : pendingAskUser.value;
  if (!ask) return null;
  if (ask.spec.intent !== "plan_approval") return null;
  const question = ask.spec.questions[0];
  return question ? { questionId: question.id, turnId: ask.turnId } : null;
});

/** orphan 模式下的 fallback cwd——延迟解析。 */
const orphanCwd = ref<string | null>(null);

async function ensureOrphanCwd(): Promise<string> {
  if (orphanCwd.value) return orphanCwd.value;
  try {
    orphanCwd.value = await homeDir();
  } catch {
    orphanCwd.value = "";
  }
  return orphanCwd.value;
}

const contextSearchCwd = computed(() => project.value?.cwd ?? orphanCwd.value ?? null);

function summarizeTitle(text: string): string {
  const normalized = text.replace(/\s+/g, " ").trim();
  if (normalized.length <= 30) return normalized;
  return normalized.slice(0, 30) + "…";
}

function titleForMessage(content: string, outgoingAttachments: ChatAttachment[]): string {
  const normalized = content.replace(/\s+/g, " ").trim();
  if (normalized) return summarizeTitle(normalized);
  return outgoingAttachments[0]?.name ?? "附件";
}

function isPointInsideElement(
  point: { x: number; y: number } | null,
  element: HTMLElement | null,
): boolean {
  if (!point || !element) return false;
  const rect = element.getBoundingClientRect();
  return point.x >= rect.left &&
    point.x <= rect.right &&
    point.y >= rect.top &&
    point.y <= rect.bottom;
}

function readDropPayload(payload: unknown): {
  type: string;
  paths: string[];
  position: { x: number; y: number } | null;
} | null {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const row = payload as Record<string, unknown>;
  const paths = Array.isArray(row.paths)
    ? row.paths.filter((path): path is string => typeof path === "string")
    : [];
  const position = row.position && typeof row.position === "object" && !Array.isArray(row.position)
    ? row.position as Record<string, unknown>
    : null;
  const x = typeof position?.x === "number" ? position.x : null;
  const y = typeof position?.y === "number" ? position.y : null;
  return {
    type: typeof row.type === "string" ? row.type : "",
    paths,
    position: x === null || y === null ? null : { x, y },
  };
}

async function addAttachmentsFromPaths(paths: string[]) {
  const uniquePaths = paths.filter((path, index) =>
    paths.indexOf(path) === index &&
    !attachments.value.some((attachment) => attachment.path === path)
  );
  if (uniquePaths.length === 0) return;
  try {
    const described = await describeAttachments(uniquePaths);
    const existing = new Set(attachments.value.map((attachment) => attachment.path));
    attachments.value = [
      ...attachments.value,
      ...described.filter((attachment) => !existing.has(attachment.path)),
    ];
  } catch (err) {
    console.error("[chat] describeAttachments failed", err);
  }
}

function addContextAttachment(attachment: ChatAttachment) {
  if (attachment.exists === false) return;
  if (attachments.value.some((item) => item.path === attachment.path)) return;
  attachments.value = [...attachments.value, attachment];
}

async function onPickAttachments() {
  try {
    const paths = await pickAttachmentFiles();
    await addAttachmentsFromPaths(paths);
  } catch (err) {
    console.error("[chat] pickAttachmentFiles failed", err);
  }
}

function removeAttachment(attachmentId: string) {
  attachments.value = attachments.value.filter((attachment) => attachment.id !== attachmentId);
}

const LILIA_GUIDE_PREFIX = "[Lilia 引导]";
const guidePriorityOrder: TaskTodoPriority[] = ["high", "normal", "low"];
const dispatchingGuideIds = new Set<string>();
let autoGuideDispatching = false;
let pendingGuideWindow: "tool" | "user" | "idle" | null = null;

function priorityLabel(priority: TaskTodoPriority): string {
  if (priority === "high") return "高";
  if (priority === "low") return "低";
  return "中";
}

function guideTextForComposer(
  content: string,
  outgoingAttachments: ChatAttachment[],
): string {
  const text = content.trim();
  if (text) return text;
  if (outgoingAttachments.length === 0) return "";
  return outgoingAttachments.map(attachmentReferenceText).join("\n");
}

function attachmentReferenceText(attachment: ChatAttachment): string {
  const kind = attachment.mime?.startsWith("image/")
    ? "图片引用"
    : attachment.kind === "directory"
      ? "目录引用"
      : "文件引用";
  return `[${kind}: ${attachment.name} | ${attachment.path}]`;
}

function guideMessage(todo: TaskTodo): string {
  return [
    LILIA_GUIDE_PREFIX,
    `优先级：${priorityLabel(todo.priority)}`,
    "",
    todo.text,
  ].join("\n");
}

async function ensureTaskReadyForMessage(
  content: string,
  outgoingAttachments: ChatAttachment[],
) {
  if (props.projectId && isDraftTask(props.taskId)) {
    await promoteDraftTask(props.taskId, titleForMessage(content, outgoingAttachments));
  } else if (!props.projectId && isDraftOrphan(props.taskId)) {
    await promoteDraftOrphan(props.taskId, titleForMessage(content, outgoingAttachments));
  }
}

async function sendAgentMessage(
  content: string,
  outgoingAttachments: ChatAttachment[] = [],
  guideId?: string,
) {
  if (!hasContext.value) return;
  if (!content.trim() && outgoingAttachments.length === 0) return;

  await ensureTaskReadyForMessage(content, outgoingAttachments);
  const cwd = project.value?.cwd ?? (await ensureOrphanCwd());

  const optimistic = createMessageTimelineEvent({
    id: `pending-${Date.now()}`,
    taskId: props.taskId,
    content,
    attachments: outgoingAttachments,
    createdAt: Date.now(),
    queued: true,
  });
  upsertTimelineEvent(optimistic);
  userSendScrollKey.value += 1;
  try {
    await sendMessage(
      props.taskId,
      content,
      composer.value,
      cwd,
      outgoingAttachments,
      guideId,
    );
    removeTimelineEvent(optimistic.id);
  } catch (err) {
    removeTimelineEvent(optimistic.id);
    isTurnRunning.value = false;
    upsertTimelineEvent(createErrorTimelineEvent(`发送失败：${String(err)}`));
    throw err;
  }
}

async function onSend(content: string, outgoingAttachments: ChatAttachment[] = []) {
  if (!hasContext.value) return;
  if (!content.trim() && outgoingAttachments.length === 0) return;

  try {
    const guideText = guideTextForComposer(content, outgoingAttachments);
    await ensureTaskReadyForMessage(guideText, outgoingAttachments);
    await createTodo(props.taskId, guideText, "normal", outgoingAttachments);
    attachments.value = [];
    if (pendingAskUsers.value.length > 0 || pendingToolConsents.value.length > 0) {
      void scheduleGuideInsertion("user");
    } else if (!isTurnRunning.value) {
      void scheduleGuideInsertion("idle");
    }
  } catch (err) {
    upsertTimelineEvent(createErrorTimelineEvent(`创建引导失败：${String(err)}`));
  }
}

async function selectPendingGuide(windowKind: "tool" | "user" | "idle"): Promise<TaskTodo | null> {
  const candidates = (await listTodos(props.taskId))
    .filter((todo) =>
      todo.source === "lilia" &&
      todo.guideStatus === "pending" &&
      !dispatchingGuideIds.has(todo.id)
    );
  const allowed = windowKind === "tool"
    ? new Set<TaskTodoPriority>(["high"])
    : windowKind === "user"
      ? new Set<TaskTodoPriority>(["normal"])
      : new Set<TaskTodoPriority>(guidePriorityOrder);
  return candidates
    .filter((todo) => allowed.has(todo.priority))
    .sort((a, b) =>
      guidePriorityOrder.indexOf(a.priority) - guidePriorityOrder.indexOf(b.priority) ||
      a.order - b.order ||
      a.createdAt - b.createdAt
    )[0] ?? null;
}

async function dispatchGuide(todo: TaskTodo) {
  if (todo.source !== "lilia" || dispatchingGuideIds.has(todo.id)) return;
  dispatchingGuideIds.add(todo.id);
  try {
    await sendAgentMessage(guideMessage(todo), todo.attachments ?? [], todo.id);
  } catch (err) {
    await updateTodo(todo.id, { guideStatus: "pending" }).catch(() => undefined);
    upsertTimelineEvent(createErrorTimelineEvent(`插入引导失败：${String(err)}`));
  } finally {
    dispatchingGuideIds.delete(todo.id);
  }
}

async function scheduleGuideInsertion(windowKind: "tool" | "user" | "idle") {
  if (autoGuideDispatching) {
    pendingGuideWindow = windowKind;
    return;
  }
  autoGuideDispatching = true;
  try {
    const guide = await selectPendingGuide(windowKind);
    if (guide) await dispatchGuide(guide);
  } catch (err) {
    upsertTimelineEvent(createErrorTimelineEvent(`调度引导失败：${String(err)}`));
  } finally {
    autoGuideDispatching = false;
    const nextWindow = pendingGuideWindow;
    pendingGuideWindow = null;
    if (nextWindow) void scheduleGuideInsertion(nextWindow);
  }
}

function onInsertGuide(todo: TaskTodo) {
  void dispatchGuide(todo);
}

async function onInterrupt() {
  if (!hasContext.value || !isTurnRunning.value) return;
  try {
    await interruptTurn(props.taskId);
  } catch (err) {
    upsertTimelineEvent(createErrorTimelineEvent(`打断失败：${String(err)}`));
  }
}

function onResolveAskUser(result: AskUserResult) {
  const ask = pendingAskUser.value;
  if (!ask) return;
  resolveAskUserById(ask.id, result);
}

async function onResolveToolConsent(
  decision: ToolConsentDecision,
  message?: string,
  updatedInput?: ToolConsentUpdatedInput,
) {
  const request = pendingToolConsent.value;
  if (!request) return;
  try {
    await respondConsent(request.taskId, request.requestId, decision, message, updatedInput);
  } catch (err) {
    console.error("[tool-consent] respond failed", err);
  }
}

async function onResolvePendingAgentAction(resolution: PendingAgentActionResolution) {
  if (resolution.kind === "tool_consent") {
    const request = pendingToolConsents.value.find(
      (item) => item.requestId === resolution.requestId,
    );
    if (!request) return;
    try {
      await respondConsent(
        request.taskId,
        request.requestId,
        resolution.decision,
        resolution.message,
        resolution.updatedInput,
      );
    } catch (err) {
      console.error("[tool-consent] respond failed", err);
    }
    return;
  }
  resolveAskUserById(resolution.askId, resolution.result);
}

async function onComposerUpdate(next: ChatComposerState) {
  const backendChanged = next.backend !== composer.value.backend;
  composer.value = next;
  if (backendChanged) {
    // 切 backend → 重拉模型清单，并把 model 修正到新清单首项。
    await reloadModelsForBackend(next.backend);
  }
  try { await setComposerState(composer.value); }
  catch (err) { console.error("[chat] setComposerState failed", err); }
}

async function reloadModelsForBackend(backend: ChatComposerState["backend"]) {
  try {
    const mdls = await listModels(backend);
    models.value = mdls;
    // 当前 model 不在新清单 → 回退首项；空清单则保留原值让后端报错。
    if (mdls.length && !mdls.some((m) => m.id === composer.value.model)) {
      composer.value = { ...composer.value, model: mdls[0].id };
    }
  } catch (err) {
    console.error("[chat] listModels failed", err);
  }
}

function upsertTimelineEvent(event: AgentTimelineEvent) {
  persistedTimelineEvents.value = upsertTimelineEventById(
    persistedTimelineEvents.value,
    event,
  );
}

function upsertOverlayTimelineEvent(event: AgentTimelineEvent) {
  overlayTimelineEvents.value = upsertTimelineEventById(
    overlayTimelineEvents.value,
    event,
  );
}

function upsertTimelineEventById(
  events: AgentTimelineEvent[],
  event: AgentTimelineEvent,
): AgentTimelineEvent[] {
  const existingIndex = events.findIndex((item) => item.id === event.id);
  if (existingIndex < 0) {
    return [...events, event];
  }
  const next = events.slice();
  next[existingIndex] = event;
  return next;
}

function removeTimelineEvent(eventId: string) {
  persistedTimelineEvents.value = persistedTimelineEvents.value.filter((item) =>
    item.id !== eventId
  );
}

function attachmentsToTimelinePayload(attachments: ChatAttachment[]): AgentTimelinePayload[] {
  return attachments.map((attachment) => ({
    id: attachment.id,
    name: attachment.name,
    path: attachment.path,
    kind: attachment.kind,
    size: attachment.size,
    exists: attachment.exists ?? null,
    mime: attachment.mime ?? null,
    directory: attachment.directory
      ? {
        fileCount: attachment.directory.fileCount,
        directoryCount: attachment.directory.directoryCount,
        totalSize: attachment.directory.totalSize,
        truncated: attachment.directory.truncated,
        unreadableCount: attachment.directory.unreadableCount,
      }
      : null,
  }));
}

function createMessageTimelineEvent(input: {
  id: string;
  taskId: string;
  content: string;
  attachments?: ChatAttachment[];
  createdAt: number;
  queued?: boolean;
}): AgentTimelineEvent {
  return {
    id: input.id,
    taskId: input.taskId,
    turnId: null,
    backend: composer.value.backend,
    kind: "message",
    status: input.queued ? "pending" : "success",
    title: "用户输入",
    summary: input.content,
    payload: {
      role: "user",
      content: input.content,
      attachments: attachmentsToTimelinePayload(input.attachments ?? []),
      queued: input.queued === true,
    },
    createdAt: input.createdAt,
    updatedAt: input.createdAt,
    // 客户端乐观事件：排在末尾占位，等同 id 的 DB 事件回来后用真实
    // turn_seq / intra_turn_order 覆盖。MAX_SAFE_INTEGER 保证乐观行不会
    // 倒插到已存在事件之间，避免短暂闪烁。
    turnSeq: Number.MAX_SAFE_INTEGER,
    intraTurnOrder: 0,
  };
}

function mergeTimelineEvents(
  events: AgentTimelineEvent[],
  current: AgentTimelineEvent[],
): AgentTimelineEvent[] {
  const byId = new Map<string, AgentTimelineEvent>();
  for (const event of events) byId.set(event.id, event);
  for (const event of current) {
    if (!byId.has(event.id)) byId.set(event.id, event);
  }
  return [...byId.values()].sort((a, b) =>
    a.turnSeq - b.turnSeq ||
    a.intraTurnOrder - b.intraTurnOrder ||
    a.createdAt - b.createdAt ||
    a.id.localeCompare(b.id)
  );
}

function createErrorTimelineEvent(message: string): AgentTimelineEvent {
  const now = Date.now();
  return {
    id: `error-${now}`,
    taskId: props.taskId,
    turnId: null,
    backend: composer.value.backend,
    kind: "error",
    status: "error",
    title: "错误",
    summary: message,
    payload: { message },
    createdAt: now,
    updatedAt: now,
    turnSeq: Number.MAX_SAFE_INTEGER,
    intraTurnOrder: Number.MAX_SAFE_INTEGER,
  };
}

async function loadTimelineEvents(taskId: string): Promise<AgentTimelineEvent[]> {
  try {
    return await listAgentTimeline(taskId);
  } catch (err) {
    console.error("[agent-timeline] list failed", err);
    return [];
  }
}

let loadSeq = 0;

async function loadAll() {
  const seq = ++loadSeq;
  const taskId = props.taskId;
  const projectId = props.projectId;
  const [events, comp] = await Promise.all([
    loadTimelineEvents(taskId),
    getComposerState(taskId),
  ]);
  if (seq !== loadSeq || taskId !== props.taskId || projectId !== props.projectId) return;
  persistedTimelineEvents.value = mergeTimelineEvents(events, persistedTimelineEvents.value);
  composer.value = comp;
  // models 依赖 backend，单独拉。
  await reloadModelsForBackend(comp.backend);
}

const unlisteners: UnlistenFn[] = [];
let unsubscribeDebugTimeline: (() => void) | null = null;
let unregisterDebugPanel: (() => void) | null = null;

function syncDebugPanelRegistration() {
  if (!hasContext.value || !agentInteractionSettings.debug.value) {
    unregisterDebugPanel?.();
    unregisterDebugPanel = null;
    return;
  }
  if (unregisterDebugPanel) return;
  unregisterDebugPanel = registerDebugChatSidebarPanel();
}

function resubscribeDebugTimeline() {
  unsubscribeDebugTimeline?.();
  unsubscribeDebugTimeline = onDebugTimelineEvent(props.taskId, upsertOverlayTimelineEvent);
}

resubscribeDebugTimeline();

onMounted(async () => {
  if (!props.projectId) await ensureOrphanCwd();
  unlisteners.push(
    await getCurrentWebview().onDragDropEvent(async (event) => {
      const drop = readDropPayload(event.payload);
      if (!drop || drop.type !== "drop" || drop.paths.length === 0) return;
      if (!isPointInsideElement(drop.position, chatPageRef.value)) return;
      await addAttachmentsFromPaths(drop.paths);
    }),
  );
  unlisteners.push(
    await onAgentTimeline((e) => {
      if (e.taskId !== props.taskId) return;
      upsertTimelineEvent(e);
      if (isAgentTimelineToolWindowKind(e.kind)) {
        void scheduleGuideInsertion("tool");
      }
    }),
  );
  unlisteners.push(
    await onTurnStarted((e) => {
      if (e.taskId !== props.taskId) return;
      isTurnRunning.value = true;
      let cleared = false;
      persistedTimelineEvents.value = persistedTimelineEvents.value.map((event) => {
        const payload = event.payload && typeof event.payload === "object" && !Array.isArray(event.payload)
          ? event.payload as Record<string, unknown>
          : {};
        if (!cleared && event.kind === "message" && payload.queued === true) {
          cleared = true;
          return {
            ...event,
            status: "success",
            payload: { ...payload, queued: false },
            updatedAt: Date.now(),
          };
        }
        return event;
      });
    }),
  );
  unlisteners.push(
    await onDone((e) => {
      if (e.taskId !== props.taskId) return;
      isTurnRunning.value = false;
      void scheduleGuideInsertion("idle");
    }),
  );
  await Promise.all([loadAll(), loadAgentInteractionSettings()]);
});

onUnmounted(async () => {
  unsubscribeDebugTimeline?.();
  unsubscribeDebugTimeline = null;
  unregisterDebugPanel?.();
  unregisterDebugPanel = null;
  for (const u of unlisteners) {
    try { await u(); } catch { /* ignore */ }
  }
  unlisteners.length = 0;
});

watch(
  () => [props.projectId, props.taskId] as const,
  async () => {
    isTurnRunning.value = false;
    persistedTimelineEvents.value = [];
    overlayTimelineEvents.value = [];
    attachments.value = [];
    resubscribeDebugTimeline();
    if (!props.projectId) await ensureOrphanCwd();
    await loadAll();
  },
);

watch(
  () => [hasContext.value, agentInteractionSettings.debug.value] as const,
  syncDebugPanelRegistration,
  { immediate: true },
);

watch(
  () => [
    pendingAskUsers.value.length,
    pendingToolConsents.value.length,
    pendingPlanApproval.value?.turnId ?? "",
  ] as const,
  ([askCount, consentCount, planTurn], [prevAskCount, prevConsentCount, prevPlanTurn]) => {
    if (
      askCount > prevAskCount ||
      consentCount > prevConsentCount ||
      (planTurn && planTurn !== prevPlanTurn)
    ) {
      void scheduleGuideInsertion("user");
    }
  },
);
</script>

<template>
  <section
    v-if="hasContext"
    ref="chatPageRef"
    class="chat-page"
  >
    <div class="chat">
      <div class="chat-layout">
        <div class="chat-layout__main">
          <ChatTranscript
            :timeline-events="timelineEvents"
            :empty-headline="emptyHeadline"
            :is-thinking="isTurnRunning"
            :project-cwd="project?.cwd ?? null"
            :active-plan-approval-turn-id="pendingPlanApproval?.turnId ?? null"
            :force-scroll-bottom-key="userSendScrollKey"
            :pending-agent-actions="pendingAgentActions"
            :show-expired-pending-actions="nonInterruptMode"
            @resolve-pending-agent-action="onResolvePendingAgentAction"
          >
            <template #controls>
              <div class="chat-controls">
                <TodoFloat
                  v-if="taskId"
                  :task-id="taskId"
                  @insert-guide="onInsertGuide"
                />
                <ChatComposer
                  :state="composer"
                  :attachments="attachments"
                  :project-cwd="contextSearchCwd"
                  :sending="isTurnRunning"
                  :pending-ask="nonInterruptMode ? null : pendingAskUser"
                  :tool-consent="nonInterruptMode ? null : pendingToolConsent"
                  @send="onSend"
                  @interrupt="onInterrupt"
                  @update:state="onComposerUpdate"
                  @remove-attachment="removeAttachment"
                  @pick-attachments="onPickAttachments"
                  @add-context-attachment="addContextAttachment"
                  @resolve-ask-user="onResolveAskUser"
                  @resolve-tool-consent="onResolveToolConsent"
                />
              </div>
            </template>
          </ChatTranscript>
        </div>
        <ChatSidebarHost
          :task-id="taskId"
          :project-id="projectId"
          :project-cwd="project?.cwd ?? null"
        />
      </div>
    </div>
  </section>

  <section v-else>
    <div class="empty-state">未找到任务 <code>{{ taskId }}</code></div>
  </section>
</template>

