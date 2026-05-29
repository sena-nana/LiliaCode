<script setup lang="ts">
import { computed } from "vue";
import AskUserPrompt from "./chat/AskUserPrompt.vue";
import { resolveAskUser, useAskUser } from "../composables/useAskUser";

const props = defineProps<{ taskId: string }>();
const { state } = useAskUser();

const current = computed(() => {
  const ask = state.current;
  if (!ask) return null;
  if (ask.taskId == null || ask.taskId === props.taskId) return ask;
  return null;
});
</script>

<template>
  <AskUserPrompt
    v-if="current"
    :key="current.id"
    :spec="current.spec"
    @resolve="resolveAskUser"
  />
</template>
