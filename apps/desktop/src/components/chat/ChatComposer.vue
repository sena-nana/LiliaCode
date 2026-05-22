<script setup lang="ts">
/**
 * Composer：textarea 自动撑高（最多 8 行）+ 一排 chip。
 * 键位：Enter 发送，Shift+Enter 换行，输入框为空时禁用发送。
 */

import { computed, nextTick, ref, watch } from "vue";
import { Paperclip, ShieldCheck, GitBranch, Sparkles, ArrowUp, Bot } from "lucide-vue-next";
import type {
  ChatBackendKind,
  ChatBranchOption,
  ChatComposerState,
  ChatModelOption,
  PermissionMode,
} from "@lilia/contracts";
import Dropdown from "../Dropdown.vue";

const props = defineProps<{
  state: ChatComposerState;
  models: ChatModelOption[];
  branches: ChatBranchOption[];
  /** 上一轮还在 streaming 时为 true，禁用发送按钮避免并发请求。 */
  sending?: boolean;
}>();

const emit = defineEmits<{
  send: [content: string];
  "update:state": [next: ChatComposerState];
}>();

const text = ref("");
const textarea = ref<HTMLTextAreaElement | null>(null);

const canSend = computed(() => !props.sending && text.value.trim().length > 0);

const permissionOptions = [
  { value: "full" as PermissionMode, label: "完全访问", hint: "无需逐条确认" },
  { value: "ask" as PermissionMode, label: "询问", hint: "敏感操作前询问" },
  { value: "readonly" as PermissionMode, label: "只读", hint: "禁止写操作" },
];

const backendOptions = [
  { value: "claude" as ChatBackendKind, label: "Claude", hint: "Claude Agent SDK" },
  { value: "codex" as ChatBackendKind, label: "Codex", hint: "OpenAI Codex SDK" },
];

const modelOptions = computed(() =>
  props.models.map((m) => ({ value: m.id, label: m.label })),
);
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

function setModel(v: string) { patch({ model: v }); }
function setBranch(v: string) { patch({ branch: v }); }
function setPermission(v: PermissionMode) { patch({ permission: v }); }
function setBackend(v: ChatBackendKind) {
  // 父层会监听 state.backend 变化重新拉 models 并修正 model 字段。
  patch({ backend: v });
}

function send() {
  const value = text.value.trim();
  if (!value) return;
  emit("send", value);
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

    <div class="chat-composer__row">
      <div class="chat-composer__group">
        <button
          type="button"
          class="chat-chip chat-chip--icon"
          title="添加附件（即将上线）"
          aria-label="添加附件"
          disabled
        >
          <Paperclip :size="14" aria-hidden="true" />
        </button>
        <Dropdown
          :model-value="state.permission"
          :options="permissionOptions"
          :icon="ShieldCheck"
          @update:model-value="setPermission"
        />
        <Dropdown
          :model-value="state.branch"
          :options="branchOptions"
          :icon="GitBranch"
          @update:model-value="setBranch"
        />
      </div>

      <div class="chat-composer__group">
        <Dropdown
          :model-value="state.backend"
          :options="backendOptions"
          :icon="Bot"
          @update:model-value="setBackend"
        />
        <Dropdown
          :model-value="state.model"
          :options="modelOptions"
          :icon="Sparkles"
          @update:model-value="setModel"
        />
        <button
          type="button"
          class="chat-composer__send"
          :disabled="!canSend"
          title="发送（Enter）"
          aria-label="发送"
          @click="send"
        >
          <ArrowUp :size="16" aria-hidden="true" />
        </button>
      </div>
    </div>
  </div>
</template>
