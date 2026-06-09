<script setup lang="ts">
import { computed } from "vue";
import { WandSparkles } from "lucide-vue-next";
import type { AgentTimelineEvent } from "@lilia/contracts";
import MarkdownBlock from "./MarkdownBlock.vue";
import type { CodexBatchApplyInput } from "./codexBatchApply";
import type { ChatImageViewerSource } from "./imageViewer";
import { readTimelinePayloadRecord, timelineFinalText } from "./timelineDisplay";

const props = withDefaults(defineProps<{
  event: AgentTimelineEvent;
  streaming?: boolean;
  canStartCodexBatchApply?: boolean;
}>(), {
  streaming: false,
  canStartCodexBatchApply: false,
});

const content = computed(() => timelineFinalText(props.event));
const hasContent = computed(() => content.value.trim().length > 0);
const isAssistantMessage = computed(() => props.event.kind === "message");
const payload = computed(() => readTimelinePayloadRecord(props.event));
const workflowSource = computed(() => {
  const source = payload.value.workflowSource;
  return source && typeof source === "object" && !Array.isArray(source)
    ? source as Record<string, unknown>
    : null;
});
const sourceKind = computed(() => {
  const value = workflowSource.value?.sourceKind;
  return value === "review" || value === "fix_suggestion" ? value : null;
});
const canApplySuggestion = computed(() =>
  props.canStartCodexBatchApply &&
  props.event.status === "success" &&
  sourceKind.value !== null &&
  hasContent.value &&
  typeof props.event.turnId === "string" &&
  props.event.turnId.length > 0,
);

const emit = defineEmits<{
  "open-image": [image: ChatImageViewerSource];
  "start-codex-batch-apply": [input: CodexBatchApplyInput];
}>();

function startCodexBatchApply() {
  if (!canApplySuggestion.value || !props.event.turnId || !sourceKind.value) return;
  emit("start-codex-batch-apply", {
    sourceTurnId: props.event.turnId,
    sourceKind: sourceKind.value,
    sourceSummary: content.value,
  });
}
</script>

<template>
  <section
    class="timeline-card timeline-card--final-reply"
    :class="{ 'is-streaming': streaming }"
    :data-agent-selectable="isAssistantMessage ? 'true' : undefined"
  >
    <MarkdownBlock
      v-if="hasContent"
      :content="content"
      class="timeline-markdown"
      @open-image="emit('open-image', $event)"
    />
    <p v-else-if="streaming" class="timeline-muted-line">
      正在生成回复…
    </p>
    <p v-else class="timeline-muted-line">
      最终回复为空。
    </p>
    <span
      v-if="streaming"
      class="timeline-card--final-reply__cursor chat-bubble__cursor"
      aria-hidden="true"
    />
    <div v-if="canApplySuggestion" class="timeline-card--final-reply__actions">
      <button
        type="button"
        class="timeline-card--final-reply__action"
        title="应用建议"
        aria-label="应用建议"
        @click="startCodexBatchApply"
      >
        <WandSparkles :size="14" aria-hidden="true" />
        <span>应用建议</span>
      </button>
    </div>
  </section>
</template>
