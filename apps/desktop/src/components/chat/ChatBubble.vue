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

const props = defineProps<{ message: ChatMessage & { streaming?: boolean; queued?: boolean } }>();
const emit = defineEmits<{
  "open-image": [image: ChatImageViewerSource];
}>();

type MessageSegment =
  | { type: "text"; text: string }
  | { type: "attachment"; attachment: ChatAttachment }
  | { type: "conversationReference"; reference: ChatConversationReference };

const referencePattern = /\[(文件引用|目录引用|图片引用): ([^\]\n|]+?) \| ([^\]\n]+?)\]/g;
const conversationReferencePattern = /\[对话引用: ([^\]\n|]+?) \| ([^\]\n]+?)\]/g;

const contentSegments = computed(() =>
  parseMessageContent(
    props.message.content,
    props.message.attachments,
    props.message.conversationReferences ?? [],
  ),
);
const previewAttachments = computed(() =>
  props.message.attachments.filter((attachment) => isImageAttachment(attachment)),
);
const unreferencedAttachments = computed(() => {
  const referenced = new Set(contentSegments.value
    .filter((segment): segment is { type: "attachment"; attachment: ChatAttachment } =>
      segment.type === "attachment"
    )
    .map((segment) => segment.attachment.path));
  return props.message.attachments.filter((attachment) =>
    !referenced.has(attachment.path) && !isImageAttachment(attachment)
  );
});

function parseMessageContent(
  content: string,
  attachments: ChatAttachment[],
  conversationReferences: ChatConversationReference[],
): MessageSegment[] {
  const segments: MessageSegment[] = [];
  let cursor = 0;
  const matches = [
    ...Array.from(content.matchAll(referencePattern), (match) => ({
      kind: "attachment" as const,
      match,
    })),
    ...Array.from(content.matchAll(conversationReferencePattern), (match) => ({
      kind: "conversationReference" as const,
      match,
    })),
  ].sort((a, b) => (a.match.index ?? 0) - (b.match.index ?? 0));
  for (const entry of matches) {
    const start = entry.match.index ?? 0;
    if (start > cursor) segments.push({ type: "text", text: content.slice(cursor, start) });
    if (entry.kind === "attachment") {
      const [, label, rawName, rawPath] = entry.match;
      const name = rawName.trim();
      const path = rawPath.trim();
      segments.push({
        type: "attachment",
        attachment: attachments.find((attachment) => attachment.path === path) ??
          fallbackAttachment(label, name, path),
      });
    } else {
      const [, rawTitle, rawTaskId] = entry.match;
      const title = rawTitle.trim();
      const taskId = rawTaskId.trim();
      segments.push({
        type: "conversationReference",
        reference: conversationReferences.find((reference) => reference.taskId === taskId) ??
          fallbackConversationReference(title, taskId),
      });
    }
    cursor = start + entry.match[0].length;
  }
  if (cursor < content.length) segments.push({ type: "text", text: content.slice(cursor) });
  return segments.length ? segments : [{ type: "text", text: content }];
}

function fallbackAttachment(label: string, name: string, path: string): ChatAttachment {
  return {
    id: `inline-${path}`,
    name: name || path,
    path,
    kind: label === "目录引用" ? "directory" : "file",
    size: null,
    exists: true,
    mime: label === "图片引用" ? "image/*" : null,
    directory: null,
  };
}

function fallbackConversationReference(title: string, taskId: string): ChatConversationReference {
  return {
    taskId,
    title: title || taskId,
    route: "",
  };
}

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
        v-for="(segment, index) in contentSegments"
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
      v-if="previewAttachments.length || unreferencedAttachments.length"
      class="chat-bubble__attachments"
      aria-label="消息附件"
    >
      <button
        v-for="attachment in previewAttachments"
        :key="attachment.id"
        type="button"
        class="chat-attachment-chip chat-attachment-chip--bubble chat-attachment-chip--image-preview"
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
        v-for="attachment in unreferencedAttachments"
        :key="attachment.id"
        class="chat-attachment-chip chat-attachment-chip--bubble"
        :title="attachment.path"
      >
        <span class="chat-attachment-chip__name">{{ attachment.name }}</span>
      </span>
    </div>
  </div>
</template>

