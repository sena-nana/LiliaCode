<script setup lang="ts">
/**
 * Task 详情 = 聊天面板。
 *
 * 之前这里有「任务信息 / 前置任务 / 子任务」三张卡，已全部移除——任务元数据
 * 后续会做成右侧抽屉/弹层独立呈现，本页只承担「人和 AI 对话」一件事。
 *
 * 数据完全走 services/chat，组件不再碰任何 stub。
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
  onAssistantMessage,
  sendMessage,
  setComposerState,
} from "../services/chat";
import type {
  ChatBranchOption,
  ChatComposerState,
  ChatMessage,
  ChatModelOption,
} from "@lilia/contracts";

const props = defineProps<{ projectId: string; taskId: string }>();

const project = computed(() => getProject(props.projectId));

const messages = ref<ChatMessage[]>([]);
const composer = ref<ChatComposerState>({
  taskId: props.taskId,
  model: "claude-sonnet-4-6",
  branch: "main",
  permission: "ask",
});
const models = ref<ChatModelOption[]>([]);
const branches = ref<ChatBranchOption[]>([]);

let unlisten: UnlistenFn | null = null;

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

async function onSend(content: string) {
  // 乐观渲染：先把 user 消息追加到本地，等命令返回拿到真实 id 再 reconcile。
  const optimistic: ChatMessage = {
    id: `pending-${Date.now()}`,
    taskId: props.taskId,
    role: "user",
    content,
    createdAt: Date.now(),
  };
  messages.value = [...messages.value, optimistic];
  try {
    const real = await sendMessage(props.taskId, content, composer.value);
    messages.value = messages.value.map((m) =>
      m.id === optimistic.id ? real : m,
    );
  } catch (err) {
    // 失败回滚 + 简单提示——日后接 toast 系统统一处理。
    messages.value = messages.value.filter((m) => m.id !== optimistic.id);
    console.error("[chat] sendMessage failed", err);
  }
}

async function onComposerUpdate(next: ChatComposerState) {
  composer.value = next;
  try {
    await setComposerState(next);
  } catch (err) {
    console.error("[chat] setComposerState failed", err);
  }
}

onMounted(async () => {
  unlisten = await onAssistantMessage((msg) => {
    // 只接受属于当前 task 的回复；切到别的 task 后老订阅未取消的极短窗口里
    // 不会把消息错误地塞进新会话。
    if (msg.taskId !== props.taskId) return;
    messages.value = [...messages.value, msg];
  });
  await loadAll();
});

onUnmounted(async () => {
  if (unlisten) {
    const fn = unlisten;
    unlisten = null;
    try { await fn(); } catch { /* ignore */ }
  }
});

/** 切换 task（侧栏点击另一个会话）时重拉数据并重置 composer。 */
watch(
  () => [props.projectId, props.taskId] as const,
  async () => {
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
        @send="onSend"
        @update:state="onComposerUpdate"
      />
    </div>
  </section>

  <section v-else>
    <div class="empty-state">未找到任务 <code>{{ taskId }}</code></div>
  </section>
</template>
