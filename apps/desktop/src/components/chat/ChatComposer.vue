<script setup lang="ts">
/**
 * Composer：textarea 自动撑高（最多 8 行）+ 一排 chip。
 * 键位：Enter 发送，Shift+Enter 换行，输入框为空时禁用发送。
 * Agent 运行中仍允许发送；消息会进入 Lilia 调度队列，等当前 turn 收束后续发。
 */

import { computed, nextTick, ref, watch } from "vue";
import { ArrowUp, GitBranch, ListChecks, Paperclip, ShieldCheck, X } from "lucide-vue-next";
import type {
  ChatAttachment,
  ChatBranchOption,
  ChatComposerState,
  PermissionMode,
} from "@lilia/contracts";
import Dropdown from "../Dropdown.vue";

const props = defineProps<{
  state: ChatComposerState;
  branches: ChatBranchOption[];
  attachments?: ChatAttachment[];
  /** 上一轮还在 streaming 时为 true，发送会进入调度队列。 */
  sending?: boolean;
  planRevisionMode?: boolean;
}>();

const emit = defineEmits<{
  send: [content: string, attachments: ChatAttachment[]];
  "update:state": [next: ChatComposerState];
  "remove-attachment": [attachmentId: string];
  "pick-attachments": [];
}>();

const text = ref("");
const textarea = ref<HTMLTextAreaElement | null>(null);

const canSend = computed(() =>
  props.planRevisionMode
    ? text.value.trim().length > 0
    : text.value.trim().length > 0 || (props.attachments?.length ?? 0) > 0,
);

const sendTitle = computed(() =>
  props.planRevisionMode
    ? "发送计划修改要求（Enter）"
    : props.sending ? "加入调度队列（Enter）" : "发送（Enter）",
);

const sendAriaLabel = computed(() =>
  props.planRevisionMode
    ? "发送计划修改要求"
    : props.sending ? "加入调度队列" : "发送",
);

const permissionOptions = [
  { value: "full" as PermissionMode, label: "完全访问", hint: "无需逐条确认" },
  { value: "ask" as PermissionMode, label: "询问", hint: "敏感操作前询问" },
  { value: "readonly" as PermissionMode, label: "只读", hint: "禁止写操作" },
];

const branchOptions = computed(() =>
  props.branches.map((b) => ({
    value: b.name,
    label: b.name,
    hint: b.current ? "当前" : undefined,
  })),
);

function patch(next: Partial<ChatComposerState>) {
  emit("update:state", { ...props.state, ...next });
}

function setBranch(v: string) { patch({ branch: v }); }
function setPermission(v: PermissionMode) { patch({ permission: v }); }
function togglePlanMode() { patch({ planMode: !props.state.planMode }); }

function send() {
  const value = text.value.trim();
  const attachments = props.planRevisionMode ? [] : props.attachments ?? [];
  if (!value && attachments.length === 0) return;
  emit("send", value, attachments);
  text.value = "";
  resize();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
    e.preventDefault();
    send();
  }
}

function resize() {
  const el = textarea.value;
  if (!el) return;
  el.style.height = "0px";
  const lineHeight = 22;
  const maxHeight = lineHeight * 8;
  el.style.height = Math.min(el.scrollHeight, maxHeight) + "px";
  el.style.overflowY = el.scrollHeight > maxHeight ? "auto" : "hidden";
}

watch(text, async () => {
  await nextTick();
  resize();
});
</script>

<template>
  <div class="chat-composer">
    <textarea
      ref="textarea"
      v-model="text"
      class="chat-composer__input"
      rows="1"
      placeholder="可向 agent 询问任何事，输入 @ 使用插件或提及文件"
      @keydown="onKeydown"
      @input="resize"
    />

    <div
      v-if="attachments?.length"
      class="chat-composer__attachments"
      aria-label="待发送附件"
    >
      <span
        v-for="attachment in attachments"
        :key="attachment.id"
        class="chat-attachment-chip"
        :title="attachment.path"
      >
        <Paperclip :size="13" aria-hidden="true" />
        <span class="chat-attachment-chip__name">{{ attachment.name }}</span>
        <button
          type="button"
          class="chat-attachment-chip__remove"
          :aria-label="`移除附件 ${attachment.name}`"
          @click="emit('remove-attachment', attachment.id)"
        >
          <X :size="12" aria-hidden="true" />
        </button>
      </span>
    </div>

    <div class="chat-composer__row">
      <div class="chat-composer__group">
        <button
          type="button"
          class="chat-chip chat-chip--icon"
          title="添加附件"
          aria-label="添加附件"
          @click="emit('pick-attachments')"
        >
          <Paperclip :size="14" aria-hidden="true" />
        </button>
        <Dropdown
          :model-value="state.permission"
          :options="permissionOptions"
          :icon="ShieldCheck"
          @update:model-value="setPermission"
        />
        <button
          type="button"
          class="chat-chip chat-chip--icon"
          :class="{ 'is-open': state.planMode }"
          :title="state.planMode ? '本轮先制定计划' : '直接执行'"
          :aria-label="state.planMode ? '关闭计划模式' : '开启计划模式'"
          :aria-pressed="state.planMode"
          @click="togglePlanMode"
        >
          <ListChecks :size="14" aria-hidden="true" />
        </button>
        <Dropdown
          :model-value="state.branch"
          :options="branchOptions"
          :icon="GitBranch"
          @update:model-value="setBranch"
        />
      </div>

      <button
        type="button"
        class="chat-composer__send"
        :disabled="!canSend"
        :title="sendTitle"
        :aria-label="sendAriaLabel"
        @click="send"
      >
        <ArrowUp :size="16" aria-hidden="true" />
      </button>
    </div>
  </div>
</template>
