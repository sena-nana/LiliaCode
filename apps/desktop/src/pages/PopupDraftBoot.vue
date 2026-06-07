<script setup lang="ts">
import { onMounted } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  createDraftOrphan,
  createDraftTask,
} from "../services/tasksStore";

const props = defineProps<{
  projectId?: string;
}>();

const router = useRouter();
const route = useRoute();

onMounted(async () => {
  if (props.projectId) {
    const draft = createDraftTask(props.projectId);
    await router.replace({
      path: `/popup/projects/${props.projectId}/tasks/${draft.id}`,
      query: route.query,
    });
    return;
  }
  const draft = createDraftOrphan();
  await router.replace({
    path: `/popup/chats/${draft.id}`,
    query: route.query,
  });
});
</script>

<template>
  <section class="empty-state">正在创建新对话…</section>
</template>
