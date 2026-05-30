<script setup lang="ts">
import type {
  AgentTimelineDisplayDetail,
  AgentTimelineDisplayField,
  AgentTimelineDisplayListItem,
} from "@lilia/contracts";
import MarkdownBlock from "./MarkdownBlock.vue";
import type { TimelineMarkdownView } from "./timelineDisplay";

defineProps<{
  details: AgentTimelineDisplayDetail[];
  fallbackView?: TimelineMarkdownView | null;
  emptyText?: string;
}>();

function detailKey(detail: AgentTimelineDisplayDetail, index: number): string {
  return `${detail.type}:${index}`;
}

function detailText(detail: AgentTimelineDisplayDetail): string {
  switch (detail.type) {
    case "line":
      return detail.text;
    case "markdown":
      return detail.content;
    case "code":
      return detail.content;
    case "fields":
      return detail.fields.map((field) => `${field.label}: ${field.value}`).join("\n");
    case "list":
      return detail.items.map((item) => item.text).join("\n");
    default:
      return "";
  }
}

function detailTone(detail: AgentTimelineDisplayDetail): "default" | "muted" {
  return detail.type === "line" || detail.type === "markdown"
    ? detail.tone === "default" ? "default" : "muted"
    : "default";
}

function detailSingleLine(detail: AgentTimelineDisplayDetail): boolean {
  if (detail.type === "line") return true;
  if (detail.type === "markdown") return detail.singleLine === true;
  return false;
}

function visibleFields(detail: AgentTimelineDisplayDetail): AgentTimelineDisplayField[] {
  return detail.type === "fields"
    ? detail.fields.filter((field) => field.label.trim() && field.value.trim())
    : [];
}

function visibleItems(detail: AgentTimelineDisplayDetail): AgentTimelineDisplayListItem[] {
  return detail.type === "list"
    ? detail.items.filter((item) => item.text.trim())
    : [];
}
</script>

<template>
  <MarkdownBlock
    v-if="fallbackView"
    :content="fallbackView.content"
    :tone="fallbackView.tone"
    :single-line="fallbackView.singleLine"
    class="timeline-muted-line"
  />

  <div
    v-if="details.length"
    class="timeline-card__details"
  >
    <template
      v-for="(detail, index) in details"
      :key="detailKey(detail, index)"
    >
      <MarkdownBlock
        v-if="detail.type === 'line' || detail.type === 'markdown'"
        :content="detailText(detail)"
        :tone="detailTone(detail)"
        :single-line="detailSingleLine(detail)"
        class="timeline-markdown"
      />

      <div
        v-else-if="detail.type === 'fields' && visibleFields(detail).length"
        class="timeline-card__field-list"
      >
        <p
          v-for="field in visibleFields(detail)"
          :key="`${field.label}:${field.value}`"
          class="timeline-card__field"
        >
          <span>{{ field.label }}</span>
          <code>{{ field.value }}</code>
        </p>
      </div>

      <section
        v-else-if="detail.type === 'code' && detail.content.trim()"
        class="timeline-card__section timeline-card__section--code"
      >
        <p v-if="detail.label" class="timeline-card__label">
          {{ detail.label }}
        </p>
        <pre class="timeline-code-block timeline-card__payload"><code>{{ detail.content }}</code></pre>
      </section>

      <section
        v-else-if="detail.type === 'list' && visibleItems(detail).length"
        class="timeline-card__section timeline-card__section--list"
      >
        <component
          :is="detail.ordered ? 'ol' : 'ul'"
          class="timeline-card__todo-list"
        >
          <li
            v-for="item in visibleItems(detail)"
            :key="item.text"
            class="timeline-card__todo-item"
          >
            <span class="timeline-card__todo-text">{{ item.text }}</span>
          </li>
        </component>
      </section>
    </template>
  </div>

  <p v-if="!fallbackView && !details.length" class="timeline-muted-line">
    {{ emptyText || "暂无摘要。" }}
  </p>
</template>
