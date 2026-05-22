<script setup lang="ts">
/**
 * Task 详情 = 聊天面板。承载两种入口：
 *   /projects/:projectId/tasks/:taskId —— 绑定到某个项目的任务对话
 *   /chats/:taskId                     —— 不绑定项目的收集箱/草稿对话
 *
 * 流式呈现：streamBuffer 缓冲 chunk，rAF 节奏的 tick 逐字 reveal 到 streaming
 * 气泡——无论 SDK 发字符级 delta 还是整段 block 最终都是「打字机」节奏。
 * projectId 缺省时进入 orphan 模式：cwd 退化到用户家目录；首次发送把草稿 promote 到收集箱。
 */

import { computed, onMounted, onUnmounted, ref, watch } from "vue";
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
  listBranches,
  listMessages,
  listModels,
  onChunk,
  onDone,
  onError,
  onTool,
  sendMessage,
  setComposerState,
} from "../services/chat";
import type {
  ChatBranchOption,
  ChatComposerState,
  ChatModelOption,
} from "@lilia/contracts";
import { useStreamReveal, type LocalMessage } from "../composables/useStreamReveal";

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
const composer = ref<ChatComposerState>({
  taskId: props.taskId,
  backend: "claude",
  model: "claude-sonnet-4-6",
  branch: "main",
  permission: "ask",
});
const models = ref<ChatModelOption[]>([]);
const branches = ref<ChatBranchOption[]>([]);

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

// 流式渲染逻辑
const {
  streamingId,
  startStreamBubble,
  abortStream,
  markStreamDone,
  appendChunk,
  stopRevealLoop,
} = useStreamReveal(messages, () => props.taskId);

function summarizeTitle(text: string): string {
  const normalized = text.replace(/\s+/g, " ").trim();
  if (normalized.length <= 30) return normalized;
  return normalized.slice(0, 30) + "…";
}

async function onSend(content: string) {
  if (!hasContext.value) return;
  if (streamingId.value) {
    // 上一轮还没结束，禁止并发请求。
    return;
  }

  // 草稿在第一条消息发出去之前先入库，即使后端报错也不撤回。
  if (props.projectId && isDraftTask(props.taskId)) {
    promoteDraftTask(props.taskId, summarizeTitle(content));
  } else if (!props.projectId && isDraftOrphan(props.taskId)) {
    promoteDraftOrphan(props.taskId, summarizeTitle(content));
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
  startStreamBubble();
  try {
    const real = await sendMessage(
      props.taskId,
      content,
      composer.value,
      cwd,
    );
    messages.value = messages.value.map((m) =>
      m.id === optimistic.id ? { ...real } : m,
    );
  } catch (err) {
    messages.value = messages.value.filter((m) => m.id !== optimistic.id);
    abortStream();
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

async function loadAll() {
  // orphan 模式没有项目分支概念，给 branches 一个空数组。
  const branchesPromise = props.projectId
    ? listBranches(props.projectId)
    : Promise.resolve<ChatBranchOption[]>([]);
  const [msgs, comp, brs] = await Promise.all([
    listMessages(props.taskId),
    getComposerState(props.taskId),
    branchesPromise,
  ]);
  messages.value = msgs;
  composer.value = comp;
  branches.value = brs;
  // models 依赖 backend，单独拉。
  await reloadModelsForBackend(comp.backend);
}

const unlisteners: UnlistenFn[] = [];

onMounted(async () => {
  unlisteners.push(
    await onChunk((e) => {
      if (e.taskId !== props.taskId) return;
      appendChunk(e.text);
    }),
  );
  unlisteners.push(
    await onTool((e) => {
      if (e.taskId !== props.taskId) return;
      pushSystemMessage(`agent 正在使用工具：${e.name}`);
    }),
  );
  unlisteners.push(
    await onDone((e) => {
      if (e.taskId !== props.taskId) return;
      markStreamDone();
    }),
  );
  unlisteners.push(
    await onError((e) => {
      if (e.taskId !== props.taskId) return;
      abortStream();
      pushSystemMessage(`agent 报错：${e.message}`);
    }),
  );
  await Promise.all([loadAll()]);
});

onUnmounted(async () => {
  stopRevealLoop();
  for (const u of unlisteners) {
    try { await u(); } catch { /* ignore */ }
  }
  unlisteners.length = 0;
});

watch(
  () => [props.projectId, props.taskId] as const,
  async () => {
    abortStream();
    messages.value = [];
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
      <ChatTranscript :messages="messages" :empty-headline="emptyHeadline" />
      <TodoFloat v-if="taskId" :task-id="taskId" />
      <ChatComposer
        :state="composer"
        :models="models"
        :branches="branches"
        :sending="streamingId !== null"
        @send="onSend"
        @update:state="onComposerUpdate"
      />
    </div>
  </section>

  <section v-else>
    <div class="empty-state">未找到任务 <code>{{ taskId }}</code></div>
  </section>
</template>

