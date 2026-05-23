<script setup lang="ts">
import { computed } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import MarkdownBlock from "./MarkdownBlock.vue";
import {
  timelineEventLabel,
  timelineFinalText,
  timelineInlinePreview,
  timelineKindLabel,
} from "./timelineDisplay";

const props = defineProps<{
  event: AgentTimelineEvent;
  processEvents?: AgentTimelineEvent[];
  processExpanded?: boolean;
}>();

const content = computed(() => timelineFinalText(props.event));
const processRows = computed(() =>
  (props.processEvents ?? []).map((event) => ({
    id: event.id,
    label: timelineKindLabel(event.kind),
    title: timelineEventLabel(event),
    preview: timelineInlinePreview(event),
  })),
);
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

    <template v-if="props.processExpanded && processRows.length">
      <div class="timeline-divider" />
      <ul class="timeline-card__process-list">
        <li
          v-for="row in processRows"
          :key="row.id"
          class="timeline-muted-line timeline-muted-line--single timeline-card__process-row"
        >
          {{ row.label }} · {{ row.title }}<span v-if="row.preview"> · {{ row.preview }}</span>
        </li>
      </ul>
    </template>
  </section>
</template>
