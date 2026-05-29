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
import ToolConsentPrompt from "../components/chat/ToolConsentPrompt.vue";
import AskUserHost from "../components/AskUserHost.vue";
import TodoFloat from "../components/todo/TodoFloat.vue";
import {
  getComposerState,
  listAgentTimeline,
  listBranches,
  listModels,
  onAgentTimeline,
  onDone,
  onTurnStarted,
  describeAttachments,
  pickAttachmentFiles,
  sendMessage,
  setComposerState,
} from "../services/chat";
import type {
  AgentTimelineEvent,
  AgentTimelinePayload,
  ChatAttachment,
  ChatBranchOption,
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

const timelineEvents = shallowRef<AgentTimelineEvent[]>([]);
const composer = ref<ChatComposerState>({
  taskId: props.taskId,
  backend: "claude",
  model: "claude-sonnet-4-6",
  branch: "main",
  planMode: false,
  permission: "ask",
});
const models = ref<ChatModelOption[]>([]);
const branches = ref<ChatBranchOption[]>([]);
const isTurnRunning = ref(false);
const chatPageRef = ref<HTMLElement | null>(null);
const attachments = ref<ChatAttachment[]>([]);

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

async function onSend(content: string, outgoingAttachments: ChatAttachment[] = []) {
  if (!hasContext.value) return;
  if (!content.trim() && outgoingAttachments.length === 0) return;

  // 草稿在第一条消息发出去之前先入库，即使后端报错也不撤回。
  if (props.projectId && isDraftTask(props.taskId)) {
    await promoteDraftTask(props.taskId, titleForMessage(content, outgoingAttachments));
  } else if (!props.projectId && isDraftOrphan(props.taskId)) {
    await promoteDraftOrphan(props.taskId, titleForMessage(content, outgoingAttachments));
  }

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
  try {
    const result = await sendMessage(
      props.taskId,
      content,
      composer.value,
      cwd,
      outgoingAttachments,
    );
    // Rust 端 chat_send_message 已经同步落库 user message timeline 事件并
    // emit 出来，前端 onAgentTimeline listener 自然会用带正确 (turnSeq,
    // intraTurnOrder) 的真实事件 upsert。这里只摘掉 pending-* 占位即可——
    // 千万不要再用本地构造的版本覆盖，否则乐观对象的 turnSeq=MAX 会把
    // 真实事件踢到时间线末尾。
    void result;
    removeTimelineEvent(optimistic.id);
    attachments.value = [];
  } catch (err) {
    removeTimelineEvent(optimistic.id);
    isTurnRunning.value = false;
    upsertTimelineEvent(createErrorTimelineEvent(`发送失败：${String(err)}`));
  }
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
  const existingIndex = timelineEvents.value.findIndex((item) => item.id === event.id);
  if (existingIndex < 0) {
    const next: AgentTimelineEvent[] = timelineEvents.value.slice();
    next.push(event);
    timelineEvents.value = next;
    return;
  }

  const next: AgentTimelineEvent[] = timelineEvents.value.slice();
  next[existingIndex] = event;
  timelineEvents.value = next;
}

function removeTimelineEvent(eventId: string) {
  timelineEvents.value = timelineEvents.value.filter((item) => item.id !== eventId);
}

function attachmentsToTimelinePayload(attachments: ChatAttachment[]): AgentTimelinePayload[] {
  return attachments.map((attachment) => ({
    id: attachment.id,
    name: attachment.name,
    path: attachment.path,
    kind: attachment.kind,
    size: attachment.size,
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
  // orphan 模式没有项目分支概念，给 branches 一个空数组。
  const branchesPromise = projectId
    ? listBranches(projectId)
    : Promise.resolve<ChatBranchOption[]>([]);
  const [events, comp, brs] = await Promise.all([
    loadTimelineEvents(taskId),
    getComposerState(taskId),
    branchesPromise,
  ]);
  if (seq !== loadSeq || taskId !== props.taskId || projectId !== props.projectId) return;
  timelineEvents.value = mergeTimelineEvents(events, timelineEvents.value);
  composer.value = comp;
  branches.value = brs;
  // models 依赖 backend，单独拉。
  await reloadModelsForBackend(comp.backend);
}

const unlisteners: UnlistenFn[] = [];

onMounted(async () => {
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
    }),
  );
  unlisteners.push(
    await onTurnStarted((e) => {
      if (e.taskId !== props.taskId) return;
      isTurnRunning.value = true;
      let cleared = false;
      timelineEvents.value = timelineEvents.value.map((event) => {
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
    }),
  );
  await Promise.all([loadAll()]);
});

onUnmounted(async () => {
  for (const u of unlisteners) {
    try { await u(); } catch { /* ignore */ }
  }
  unlisteners.length = 0;
});

watch(
  () => [props.projectId, props.taskId] as const,
  async () => {
    isTurnRunning.value = false;
    timelineEvents.value = [];
    attachments.value = [];
    await loadAll();
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
      <ChatTranscript
        :timeline-events="timelineEvents"
        :empty-headline="emptyHeadline"
        :is-thinking="isTurnRunning"
      >
        <template #controls>
          <div class="chat-controls">
            <TodoFloat v-if="taskId" :task-id="taskId" />
            <AskUserHost :task-id="taskId" />
            <ToolConsentPrompt :task-id="taskId" />
            <ChatComposer
              :state="composer"
              :branches="branches"
              :attachments="attachments"
              :sending="isTurnRunning"
              @send="onSend"
              @update:state="onComposerUpdate"
              @remove-attachment="removeAttachment"
              @pick-attachments="onPickAttachments"
            />
          </div>
        </template>
      </ChatTranscript>
    </div>
  </section>

  <section v-else>
    <div class="empty-state">未找到任务 <code>{{ taskId }}</code></div>
  </section>
</template>

