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

function queryString(value: unknown): string | null {
  if (Array.isArray(value)) return typeof value[0] === "string" ? value[0] : null;
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function decodeQueryText(value: string | null): string | null {
  if (!value) return null;
  try {
    const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
    const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "=");
    const binary = atob(padded);
    const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
    return new TextDecoder().decode(bytes);
  } catch {
    return value;
  }
}

onMounted(async () => {
  const parentId = decodeQueryText(queryString(route.query.parentTaskId));
  if (props.projectId) {
    const draft = createDraftTask(props.projectId, parentId);
    await router.replace({
      path: `/popup/projects/${props.projectId}/tasks/${draft.id}`,
      query: route.query,
    });
    return;
  }
  const draft = createDraftOrphan(parentId);
  await router.replace({
    path: `/popup/chats/${draft.id}`,
    query: route.query,
  });
});
</script>

<template>
  <section class="empty-state">正在创建新对话…</section>
</template>

