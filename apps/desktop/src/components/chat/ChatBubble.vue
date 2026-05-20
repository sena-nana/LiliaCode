<script setup lang="ts">
import type { ChatMessage } from "@lilia/contracts";

defineProps<{ message: ChatMessage & { streaming?: boolean } }>();
</script>

<template>
  <div
    class="chat-bubble"
    :class="[`chat-bubble--${message.role}`, { 'is-streaming': message.streaming }]"
    :data-role="message.role"
  >
    <div class="chat-bubble__content">{{ message.content }}<span
      v-if="message.streaming"
      class="chat-bubble__cursor"
      aria-hidden="true"
    /></div>
  </div>
</template>

