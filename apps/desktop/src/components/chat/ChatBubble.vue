<script setup lang="ts">
import type { ChatMessage } from "@lilia/contracts";

defineProps<{ message: ChatMessage & { streaming?: boolean; queued?: boolean } }>();
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
    <div v-if="message.content" class="chat-bubble__content">{{ message.content }}<span
      v-if="message.streaming"
      class="chat-bubble__cursor"
      aria-hidden="true"
    /></div>
    <div
      v-if="message.attachments.length"
      class="chat-bubble__attachments"
      aria-label="消息附件"
    >
      <span
        v-for="attachment in message.attachments"
        :key="attachment.id"
        class="chat-attachment-chip chat-attachment-chip--bubble"
        :title="attachment.path"
      >
        <span class="chat-attachment-chip__name">{{ attachment.name }}</span>
      </span>
    </div>
  </div>
</template>

