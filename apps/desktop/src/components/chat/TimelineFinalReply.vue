<script setup lang="ts">
import { computed } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import MarkdownBlock from "./MarkdownBlock.vue";
import { timelineFinalText } from "./timelineDisplay";

const props = withDefaults(defineProps<{
  event: AgentTimelineEvent;
  streaming?: boolean;
}>(), {
  streaming: false,
});

const content = computed(() => timelineFinalText(props.event));
const hasContent = computed(() => content.value.trim().length > 0);
</script>

<template>
  <section
    class="timeline-card timeline-card--final-reply"
    :class="{ 'is-streaming': streaming }"
  >
    <MarkdownBlock
      v-if="hasContent"
      :content="content"
      class="timeline-markdown"
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
  </section>
</template>
