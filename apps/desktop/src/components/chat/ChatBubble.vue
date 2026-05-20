<script setup lang="ts">
import type { ChatMessage } from "@lilia/contracts";

defineProps<{ message: ChatMessage }>();
</script>

<template>
  <div
    class="chat-bubble"
    :class="`chat-bubble--${message.role}`"
    :data-role="message.role"
  >
    <div class="chat-bubble__content">{{ message.content }}</div>
  </div>
</template>
