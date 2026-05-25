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
import TodoFloat from "../components/todo/TodoFloat.vue";
import {
  getComposerState,
  listAgentTimeline,
  listBranches,
  listModels,
  onAgentTimeline,
  onDone,
  onTurnStarted,
  sendMessage,
  setComposerState,
} from "../services/chat";
import type {
  AgentTimelineEvent,
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
  permission: "ask",
});
const models = ref<ChatModelOption[]>([]);
const branches = ref<ChatBranchOption[]>([]);
const isTurnRunning = ref(false);

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

async function onSend(content: string) {
  if (!hasContext.value) return;

  // 草稿在第一条消息发出去之前先入库，即使后端报错也不撤回。
  if (props.projectId && isDraftTask(props.taskId)) {
    await promoteDraftTask(props.taskId, summarizeTitle(content));
  } else if (!props.projectId && isDraftOrphan(props.taskId)) {
    await promoteDraftOrphan(props.taskId, summarizeTitle(content));
  }

  const cwd = project.value?.cwd ?? (await ensureOrphanCwd());

  const optimistic = createMessageTimelineEvent({
    id: `pending-${Date.now()}`,
    taskId: props.taskId,
    content,
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
    );
    removeTimelineEvent(optimistic.id);
    upsertTimelineEvent(createMessageTimelineEvent({
      id: result.message.id,
      taskId: result.message.taskId,
      content: result.message.content,
      createdAt: result.message.createdAt,
      queued: result.dispatch === "queued",
    }));
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

function createMessageTimelineEvent(input: {
  id: string;
  taskId: string;
  content: string;
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
      queued: input.queued === true,
    },
    display: {
      icon: "message",
      label: "用户输入",
      preview: input.content,
    },
    createdAt: input.createdAt,
    updatedAt: input.createdAt,
    order: 0,
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
    a.createdAt - b.createdAt || a.order - b.order || a.id.localeCompare(b.id)
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
    display: {
      icon: "error",
      label: "错误",
      preview: message,
      details: [
        { type: "line", text: message, tone: "muted" },
      ],
      group: {
        key: "kind:error",
        bucket: "error",
        unit: "个错误",
        count: 1,
      },
    },
    createdAt: now,
    updatedAt: now,
    order: Number.MAX_SAFE_INTEGER,
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
    await loadAll();
  },
);
</script>

<template>
  <section
    v-if="hasContext"
    class="chat-page"
  >
    <div class="chat">
      <ChatTranscript
        :timeline-events="timelineEvents"
        :empty-headline="emptyHeadline"
      />
      <TodoFloat v-if="taskId" :task-id="taskId" />
      <ChatComposer
        :state="composer"
        :models="models"
        :branches="branches"
        :sending="isTurnRunning"
        @send="onSend"
        @update:state="onComposerUpdate"
      />
    </div>
  </section>

  <section v-else>
    <div class="empty-state">未找到任务 <code>{{ taskId }}</code></div>
  </section>
</template>

