<script setup lang="ts">
import { computed } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { FileText, Folder, Image, Paperclip } from "lucide-vue-next";
import type { ChatAttachment, ChatMessage } from "@lilia/contracts";

const props = defineProps<{ message: ChatMessage & { streaming?: boolean; queued?: boolean } }>();

type MessageSegment =
  | { type: "text"; text: string }
  | { type: "attachment"; attachment: ChatAttachment };

const referencePattern = /\[(文件引用|目录引用|图片引用): ([^\]\n|]+?) \| ([^\]\n]+?)\]/g;

const contentSegments = computed(() =>
  parseMessageContent(props.message.content, props.message.attachments),
);
const previewAttachments = computed(() =>
  props.message.attachments.filter((attachment) => isImageAttachment(attachment)),
);
const legacyAttachments = computed(() => {
  const referenced = new Set(contentSegments.value
    .filter((segment): segment is { type: "attachment"; attachment: ChatAttachment } =>
      segment.type === "attachment"
    )
    .map((segment) => segment.attachment.path));
  return props.message.attachments.filter((attachment) =>
    !referenced.has(attachment.path) && !isImageAttachment(attachment)
  );
});

function parseMessageContent(content: string, attachments: ChatAttachment[]): MessageSegment[] {
  const segments: MessageSegment[] = [];
  let cursor = 0;
  for (const match of content.matchAll(referencePattern)) {
    const start = match.index ?? 0;
    if (start > cursor) segments.push({ type: "text", text: content.slice(cursor, start) });
    const [, label, rawName, rawPath] = match;
    const name = rawName.trim();
    const path = rawPath.trim();
    segments.push({
      type: "attachment",
      attachment: attachments.find((attachment) => attachment.path === path) ??
        fallbackAttachment(label, name, path),
    });
    cursor = start + match[0].length;
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

function isImageAttachment(attachment: ChatAttachment): boolean {
  return attachment.exists !== false && !!attachment.mime?.startsWith("image/");
}

function attachmentImageSrc(attachment: ChatAttachment): string | null {
  if (!isImageAttachment(attachment)) return null;
  try {
    return convertFileSrc(attachment.path);
  } catch {
    return null;
  }
}

function attachmentIcon(attachment: ChatAttachment) {
  if (isImageAttachment(attachment)) return Image;
  if (attachment.kind === "directory") return Folder;
  if (attachment.kind === "file") return FileText;
  return Paperclip;
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
          v-else
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
      </template>
      <span
        v-if="message.streaming"
        class="chat-bubble__cursor"
        aria-hidden="true"
      />
    </div>
    <div
      v-if="previewAttachments.length || legacyAttachments.length"
      class="chat-bubble__attachments"
      aria-label="消息附件"
    >
      <span
        v-for="attachment in previewAttachments"
        :key="attachment.id"
        class="chat-attachment-chip chat-attachment-chip--bubble"
        :title="attachment.path"
      >
        <img
          class="chat-attachment-chip__thumb"
          :src="attachmentImageSrc(attachment) ?? undefined"
          alt=""
        />
        <span class="chat-attachment-chip__name">{{ attachment.name }}</span>
      </span>
      <span
        v-for="attachment in legacyAttachments"
        :key="attachment.id"
        class="chat-attachment-chip chat-attachment-chip--bubble"
        :title="attachment.path"
      >
        <span class="chat-attachment-chip__name">{{ attachment.name }}</span>
      </span>
    </div>
  </div>
</template>

