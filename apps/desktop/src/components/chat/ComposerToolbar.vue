<script setup lang="ts">
import { ArrowUp, ListChecks, Paperclip, ShieldCheck, Square } from "lucide-vue-next";
import type { ChatAttachment, ChatComposerState, PermissionMode } from "@lilia/contracts";
import Dropdown from "../Dropdown.vue";
import { attachmentImageSrc } from "./imageViewer";

defineProps<{
  state: ChatComposerState;
  permissionOptions: Array<{ value: PermissionMode; label: string; hint: string }>;
  previewAttachments: ChatAttachment[];
  canInterrupt: boolean;
  canSubmitEntry: boolean;
  sendTitle: string;
  sendAriaLabel: string;
}>();

const emit = defineEmits<{
  pickAttachments: [];
  setPermission: [permission: PermissionMode];
  togglePlanMode: [];
  submitEntry: [];
  openImage: [attachment: ChatAttachment];
}>();
</script>

<template>
  <div class="chat-composer__toolbar">
    <div
      v-if="previewAttachments.length"
      class="chat-composer__attachments"
      aria-label="图片预览"
    >
      <button
        v-for="attachment in previewAttachments"
        :key="attachment.id"
        type="button"
        class="chat-attachment-chip chat-attachment-chip--image-preview"
        :title="attachment.path"
        :aria-label="`查看图片 ${attachment.name}`"
        @click="emit('openImage', attachment)"
      >
        <img
          class="chat-attachment-chip__thumb"
          :src="attachmentImageSrc(attachment) ?? undefined"
          alt=""
        />
      </button>
    </div>

    <div class="chat-composer__row">
      <div class="chat-composer__group">
        <button
          type="button"
          class="chat-chip chat-chip--icon"
          title="添加附件"
          aria-label="添加附件"
          @click="emit('pickAttachments')"
        >
          <Paperclip :size="14" aria-hidden="true" />
        </button>
        <Dropdown
          :model-value="state.permission"
          :options="permissionOptions"
          :icon="ShieldCheck"
          @update:model-value="emit('setPermission', $event)"
        />
        <button
          type="button"
          class="chat-chip chat-chip--icon"
          :class="{ 'is-open': state.planMode }"
          :title="state.planMode ? '本轮先制定计划' : '直接执行'"
          :aria-label="state.planMode ? '关闭计划模式' : '开启计划模式'"
          :aria-pressed="state.planMode"
          @click="emit('togglePlanMode')"
        >
          <ListChecks :size="14" aria-hidden="true" />
        </button>
      </div>

      <button
        type="button"
        class="chat-composer__send"
        :class="{ 'chat-composer__send--interrupt': canInterrupt }"
        :disabled="!canSubmitEntry"
        :title="sendTitle"
        :aria-label="sendAriaLabel"
        @click="emit('submitEntry')"
      >
        <component :is="canInterrupt ? Square : ArrowUp" :size="16" aria-hidden="true" />
      </button>
    </div>
  </div>
</template>
