<script setup lang="ts">
/**
 * Task 详情 = 聊天面板。承载两种入口：
 *   /projects/:projectId/tasks/:taskId —— 绑定到某个项目的任务对话
 *   /chats/:taskId                     —— 不绑定项目的收集箱/草稿对话
 *
 * Agent 过程和最终回复走 timeline 呈现；transcript 里的气泡只保留用户输入
 * 和必要的 system 错误提示，避免 assistant 回复跑到过程时间线前面。
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
  listMessages,
  listModels,
  onAgentTimeline,
  onDone,
  onError,
  onTurnStarted,
  sendMessage,
  setComposerState,
} from "../services/chat";
import type {
  AgentTimelineEvent,
  ChatBranchOption,
  ChatMessage,
  ChatComposerState,
  ChatModelOption,
} from "@lilia/contracts";

type LocalMessage = ChatMessage & { queued?: boolean };

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

const messages = ref<LocalMessage[]>([]);
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

function isVisibleMessage(message: ChatMessage): boolean {
  return message.role !== "assistant";
}

function sortMessages(list: LocalMessage[]): LocalMessage[] {
  return [...list].sort((a, b) =>
    a.createdAt - b.createdAt || a.id.localeCompare(b.id)
  );
}

function mergeVisibleMessages(
  fetched: ChatMessage[],
  current: LocalMessage[],
): LocalMessage[] {
  const byId = new Map<string, LocalMessage>();
  for (const message of fetched) {
    if (isVisibleMessage(message)) byId.set(message.id, message);
  }
  for (const message of current) {
    if (isVisibleMessage(message) && !byId.has(message.id)) {
      byId.set(message.id, message);
    }
  }
  return sortMessages([...byId.values()]);
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

  const optimistic: LocalMessage = {
    id: `pending-${Date.now()}`,
    taskId: props.taskId,
    role: "user",
    content,
    createdAt: Date.now(),
  };
  messages.value = [...messages.value, optimistic];
  try {
    const result = await sendMessage(
      props.taskId,
      content,
      composer.value,
      cwd,
    );
    let replaced = false;
    messages.value = messages.value.map((m) => {
      if (m.id !== optimistic.id) return m;
      replaced = true;
      return { ...result.message, queued: result.dispatch === "queued" };
    });
    if (!replaced && isVisibleMessage(result.message)) {
      messages.value = sortMessages([
        ...messages.value,
        { ...result.message, queued: result.dispatch === "queued" },
      ]);
    }
  } catch (err) {
    messages.value = messages.value.filter((m) => m.id !== optimistic.id);
    isTurnRunning.value = false;
    pushSystemMessage(`发送失败：${String(err)}`);
  }
}

function pushSystemMessage(text: string) {
  messages.value = [
    ...messages.value,
    {
      id: `sys-${Date.now()}`,
      taskId: props.taskId,
      role: "system",
      content: text,
      createdAt: Date.now(),
    },
  ];
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
  const [msgs, events, comp, brs] = await Promise.all([
    listMessages(taskId),
    loadTimelineEvents(taskId),
    getComposerState(taskId),
    branchesPromise,
  ]);
  if (seq !== loadSeq || taskId !== props.taskId || projectId !== props.projectId) return;
  messages.value = mergeVisibleMessages(msgs, messages.value);
  timelineEvents.value = events;
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
      messages.value = messages.value.map((m) => {
        if (!cleared && m.queued && m.role === "user") {
          cleared = true;
          return { ...m, queued: false };
        }
        return m;
      });
    }),
  );
  unlisteners.push(
    await onDone((e) => {
      if (e.taskId !== props.taskId) return;
      isTurnRunning.value = false;
    }),
  );
  unlisteners.push(
    await onError((e) => {
      if (e.taskId !== props.taskId) return;
      isTurnRunning.value = false;
      pushSystemMessage(`agent 报错：${e.message}`);
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
    messages.value = [];
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
        :messages="messages"
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

