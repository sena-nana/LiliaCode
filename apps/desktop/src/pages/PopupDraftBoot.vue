<script setup lang="ts">
import { onMounted } from "vue";
import { useRouter } from "vue-router";
import {
  createDraftOrphan,
  createDraftTask,
} from "../services/tasksStore";

const props = defineProps<{
  projectId?: string;
}>();

const router = useRouter();

onMounted(async () => {
  if (props.projectId) {
    const draft = createDraftTask(props.projectId);
    await router.replace(`/popup/projects/${props.projectId}/tasks/${draft.id}`);
    return;
  }
  const draft = createDraftOrphan();
  await router.replace(`/popup/chats/${draft.id}`);
});
</script>

<template>
  <section class="empty-state">正在创建新对话…</section>
</template>
