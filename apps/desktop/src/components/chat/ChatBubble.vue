<script setup lang="ts">
import { computed } from "vue";
import { FileText, Folder, Hash, Image, Paperclip } from "lucide-vue-next";
import type { ChatAttachment, ChatConversationReference, ChatMessage } from "@lilia/contracts";
import {
  attachmentImageSrc,
  imageViewerSourceFromAttachment,
  isImageAttachment,
  type ChatImageViewerSource,
} from "./imageViewer";
import { readChatBubbleDisplay } from "./chatBubbleDisplay";

const props = defineProps<{ message: ChatMessage & { streaming?: boolean; queued?: boolean } }>();
const emit = defineEmits<{
  "open-image": [image: ChatImageViewerSource];
}>();

const bubbleDisplay = computed(() => readChatBubbleDisplay(props.message));

function openAttachmentImage(attachment: ChatAttachment) {
  const source = imageViewerSourceFromAttachment(attachment);
  if (source) emit("open-image", source);
}

function attachmentIcon(attachment: ChatAttachment) {
  if (isImageAttachment(attachment)) return Image;
  if (attachment.kind === "directory") return Folder;
  if (attachment.kind === "file") return FileText;
  return Paperclip;
}

function conversationReferenceScope(reference: ChatConversationReference) {
  return reference.projectName ?? "收集箱";
}
</script>

<template>
  <div
    class="chat-bubble"
    :class="[
      `chat-bubble--${message.role}`,
      { 'is-streaming': message.streaming, 'is-queued': message.queued },
    ]"
    :data-role="message.role"
  >
    <div v-if="message.content" class="chat-bubble__content">
      <template
        v-for="(segment, index) in bubbleDisplay.segments"
        :key="index"
      >
        <span v-if="segment.type === 'text'">{{ segment.text }}</span>
        <span
          v-else-if="segment.type === 'attachment'"
          class="chat-file-reference chat-file-reference--readonly"
          :title="segment.attachment.path"
        >
          <span class="chat-file-reference__icon" aria-hidden="true">
            <component :is="attachmentIcon(segment.attachment)" :size="12" />
          </span>
          <span class="chat-file-reference__main">
            <span class="chat-file-reference__name">{{ segment.attachment.name }}</span>
          </span>
        </span>
        <span
          v-else
          class="chat-file-reference chat-file-reference--readonly chat-file-reference--conversation"
          :title="segment.reference.route || segment.reference.title"
        >
          <span class="chat-file-reference__icon" aria-hidden="true">
            <Hash :size="12" />
          </span>
          <span class="chat-file-reference__main">
            <span class="chat-file-reference__name">{{ segment.reference.title }}</span>
          </span>
          <span class="chat-file-reference__meta">
            {{ conversationReferenceScope(segment.reference) }}
          </span>
        </span>
      </template>
      <span
        v-if="message.streaming"
        class="chat-bubble__cursor"
        aria-hidden="true"
      />
    </div>
    <div
      v-if="bubbleDisplay.previewAttachments.length || bubbleDisplay.unreferencedAttachments.length"
      class="chat-bubble__attachments"
      aria-label="消息附件"
    >
      <button
        v-for="attachment in bubbleDisplay.previewAttachments"
        :key="attachment.id"
        type="button"
        class="chat-attachment-chip chat-attachment-chip--bubble chat-attachment-chip--image-preview"
        :data-agent-id="`chat.bubble.attachment.${attachment.id}`"
        :title="attachment.path"
        :aria-label="`查看图片 ${attachment.name}`"
        @click="openAttachmentImage(attachment)"
      >
        <img
          class="chat-attachment-chip__thumb"
          :src="attachmentImageSrc(attachment) ?? undefined"
          alt=""
        />
      </button>
      <span
        v-for="attachment in bubbleDisplay.unreferencedAttachments"
        :key="attachment.id"
        class="chat-attachment-chip chat-attachment-chip--bubble"
        :title="attachment.path"
      >
        <span class="chat-attachment-chip__name">{{ attachment.name }}</span>
      </span>
    </div>
  </div>
</template>

