<script setup lang="ts">
import { computed } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import MarkdownBlock from "./MarkdownBlock.vue";
import { timelineFinalText } from "./timelineDisplay";

const props = defineProps<{
  event: AgentTimelineEvent;
}>();

const content = computed(() => timelineFinalText(props.event));
</script>

<template>
  <section class="timeline-card timeline-card--final-reply">
    <MarkdownBlock
      v-if="content"
      :content="content"
      class="timeline-markdown"
    />
    <p v-else class="timeline-muted-line">
      最终回复为空。
    </p>
  </section>
</template>
