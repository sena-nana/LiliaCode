<script setup lang="ts">
/**
 * Task 详情 = 聊天面板。
 *
 * 数据全程走 services/chat：用户消息走 invoke 直接落盘到后端历史，
 * assistant 回复通过 chat:chunk / chat:done / chat:error 流式推回。
 *
 * 流式呈现：维护一个 streamBuffer 把后端来的文本缓冲起来，一个 rAF 节奏的
 * tick 把它逐字 reveal 到当前 streaming 气泡的 content 上——无论 SDK 发的
 * 是字符级 delta 还是整段 block，最终呈现都是「打字机」节奏。
 */

import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getProject } from "../data/projectsStub";
import ViewTabs from "../components/ViewTabs.vue";
import ChatTranscript from "../components/chat/ChatTranscript.vue";
import ChatComposer from "../components/chat/ChatComposer.vue";
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
  ChatMessage,
  ChatModelOption,
} from "@lilia/contracts";

type LocalMessage = ChatMessage & {
  /** 还在打字，最后一个 assistant 气泡才会是 true。 */
  streaming?: boolean;
};

const props = defineProps<{ projectId: string; taskId: string }>();

const project = computed(() => getProject(props.projectId));

const messages = ref<LocalMessage[]>([]);
const composer = ref<ChatComposerState>({
  taskId: props.taskId,
  model: "claude-sonnet-4-6",
  branch: "main",
  permission: "ask",
});
const models = ref<ChatModelOption[]>([]);
const branches = ref<ChatBranchOption[]>([]);

// 流式状态——所有 timer / buffer 都按 taskId 隔离，切 task 时一并清。
const streamBuffer = ref("");
const streamingId = ref<string | null>(null);
let streamFinalized = false;
let revealTimer: number | null = null;

const unlisteners: UnlistenFn[] = [];

function startStreamBubble() {
  // 在历史尾部加一条空的 assistant 气泡，后续 chunk 都 reveal 到它的 content 上。
  const bubble: LocalMessage = {
    id: `stream-${Date.now()}`,
    taskId: props.taskId,
    role: "assistant",
    content: "",
    createdAt: Date.now(),
    streaming: true,
  };
  messages.value = [...messages.value, bubble];
  streamingId.value = bubble.id;
  streamBuffer.value = "";
  streamFinalized = false;
  ensureRevealLoop();
}

function ensureRevealLoop() {
  if (revealTimer !== null) return;
  revealTimer = window.setInterval(tickReveal, 24);
}

function stopRevealLoop() {
  if (revealTimer !== null) {
    window.clearInterval(revealTimer);
    revealTimer = null;
  }
}

function tickReveal() {
  if (!streamingId.value) {
    stopRevealLoop();
    return;
  }
  if (streamBuffer.value.length === 0) {
    if (streamFinalized) finalizeStream();
    return;
  }
  // 一次 reveal 多少：随缓冲规模放大，避免长回复看起来像便秘。
  const n = Math.max(
    1,
    Math.min(streamBuffer.value.length, Math.ceil(streamBuffer.value.length / 20)),
  );
  const slice = streamBuffer.value.slice(0, n);
  streamBuffer.value = streamBuffer.value.slice(n);
  const idx = messages.value.findIndex((m) => m.id === streamingId.value);
  if (idx >= 0) {
    const m = messages.value[idx];
    messages.value[idx] = { ...m, content: m.content + slice };
  }
}

function finalizeStream() {
  const idx = messages.value.findIndex((m) => m.id === streamingId.value);
  if (idx >= 0) {
    messages.value[idx] = { ...messages.value[idx], streaming: false };
  }
  streamingId.value = null;
  streamFinalized = false;
  stopRevealLoop();
}

function abortStream() {
  // 切 task 时遗留的流式气泡也清掉——避免下个会话里看到上一个会话的半截回复。
  if (streamingId.value) {
    messages.value = messages.value.filter((m) => m.id !== streamingId.value);
  }
  streamingId.value = null;
  streamBuffer.value = "";
  streamFinalized = false;
  stopRevealLoop();
}

async function onSend(content: string) {
  if (!project.value) return;
  if (streamingId.value) {
    // 上一轮还没结束，禁止并发请求——后续可以做「中断 + 重发」。
    return;
  }
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
      project.value.cwd,
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
  composer.value = next;
  try { await setComposerState(next); }
  catch (err) { console.error("[chat] setComposerState failed", err); }
}

async function loadAll() {
  const [msgs, comp, mdls, brs] = await Promise.all([
    listMessages(props.taskId),
    getComposerState(props.taskId),
    listModels(),
    listBranches(props.projectId),
  ]);
  messages.value = msgs;
  composer.value = comp;
  models.value = mdls;
  branches.value = brs;
}

onMounted(async () => {
  unlisteners.push(
    await onChunk((e) => {
      if (e.taskId !== props.taskId) return;
      // 收到的文本进缓冲池，由 tickReveal 慢慢吐到气泡上。
      streamBuffer.value += e.text;
      ensureRevealLoop();
    }),
  );
  unlisteners.push(
    await onTool((e) => {
      if (e.taskId !== props.taskId) return;
      // 第一阶段只展示工具名；input 摘要等后续做。
      pushSystemMessage(`Claude 正在使用工具：${e.name}`);
    }),
  );
  unlisteners.push(
    await onDone((e) => {
      if (e.taskId !== props.taskId) return;
      streamFinalized = true;
      ensureRevealLoop();
    }),
  );
  unlisteners.push(
    await onError((e) => {
      if (e.taskId !== props.taskId) return;
      abortStream();
      pushSystemMessage(`Claude 报错：${e.message}`);
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
  <section v-if="project" class="chat-page">
    <ViewTabs :project-id="projectId" active="sessions" />
    <div class="chat">
      <ChatTranscript :messages="messages" :project-name="project.name" />
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
