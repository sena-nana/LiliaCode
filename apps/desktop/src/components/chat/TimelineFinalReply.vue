<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";
import { GitFork, WandSparkles } from "lucide-vue-next";
import {
  timelineFinalReplyBatchApplyInput,
  type AgentTimelineEvent,
  type LiliaBatchApplyInput,
} from "@lilia/contracts";
import type { ChatImageViewerSource } from "./imageViewer";
import { measurePerfAsync } from "../../utils/perf";
import { isTimelineMessageEvent, timelineFinalText } from "./timelineDisplay";

const MarkdownBlock = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "timeline.markdown.load",
    async () => (await import("./MarkdownBlock.vue")).default,
  ),
});

const props = withDefaults(defineProps<{
  event: AgentTimelineEvent;
  streaming?: boolean;
  canStartLiliaBatchApply?: boolean;
  canStartSessionFork?: boolean;
}>(), {
  streaming: false,
  canStartLiliaBatchApply: false,
  canStartSessionFork: false,
});

const content = computed(() => timelineFinalText(props.event));
const hasContent = computed(() => content.value.trim().length > 0);
const isAssistantMessage = computed(() => isTimelineMessageEvent(props.event));
const batchApplyInput = computed(() =>
  timelineFinalReplyBatchApplyInput(props.event, content.value)
);
const canApplySuggestion = computed(() =>
  props.canStartLiliaBatchApply &&
  batchApplyInput.value !== null,
);
const canStartSessionFork = computed(() =>
  props.canStartSessionFork &&
  !props.streaming &&
  props.event.status === "success" &&
  isTimelineMessageEvent(props.event),
);

const emit = defineEmits<{
  "open-image": [image: ChatImageViewerSource];
  "start-lilia-batch-apply": [input: LiliaBatchApplyInput];
  "start-session-fork": [];
}>();

function startLiliaBatchApply() {
  const input = batchApplyInput.value;
  if (!canApplySuggestion.value || !input) return;
  emit("start-lilia-batch-apply", input);
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
    <div
      v-if="canApplySuggestion || canStartSessionFork"
      class="timeline-card--final-reply__actions"
      :class="{ 'has-visible-action': canApplySuggestion }"
    >
      <button
        v-if="canStartSessionFork"
        type="button"
        class="timeline-card--final-reply__action timeline-card--final-reply__action--ghost"
        title="分叉当前会话"
        aria-label="分叉当前会话"
        @click="emit('start-session-fork')"
      >
        <GitFork :size="14" aria-hidden="true" />
      </button>
      <button
        v-if="canApplySuggestion"
        type="button"
        class="timeline-card--final-reply__action"
        title="应用建议"
        aria-label="应用建议"
        @click="startLiliaBatchApply"
      >
        <WandSparkles :size="14" aria-hidden="true" />
        <span>应用建议</span>
      </button>
    </div>
  </section>
</template>
