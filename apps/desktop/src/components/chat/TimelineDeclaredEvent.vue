<script setup lang="ts">
import { computed } from "vue";
import type {
  AgentTimelineDisplayDetail,
  AgentTimelineDisplayField,
  AgentTimelineDisplayListItem,
  AgentTimelineEvent,
} from "@lilia/contracts";
import MarkdownBlock from "./MarkdownBlock.vue";
import {
  createTimelineMarkdownView,
  readTimelineDisplay,
  truncateTimelineText,
} from "./timelineDisplay";

const props = defineProps<{
  event: AgentTimelineEvent;
  expanded: boolean;
  compact?: boolean;
}>();

const display = computed(() => readTimelineDisplay(props.event));
const details = computed(() => display.value.details ?? []);
const collapsed = computed(() => props.compact === true || !props.expanded);
const summaryLine = computed(() =>
  display.value.preview?.trim() ||
  display.value.label?.trim() ||
  "暂无摘要。",
);
const compactView = computed(() =>
  createTimelineMarkdownView(
    truncateTimelineText(summaryLine.value, 180),
    { forceSingleLine: true, singleLineTone: "muted" },
  ),
);
const expandedFallbackView = computed(() =>
  details.value.length
    ? null
    : createTimelineMarkdownView(summaryLine.value, {
        multilineTone: "muted",
        singleLineTone: "muted",
      }),
);

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
  if (detail.type === "line" || detail.type === "markdown") {
    return detail.tone === "default" ? "default" : "muted";
  }
  return "default";
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
  <article
    class="timeline-card timeline-card--declared"
    :class="{ 'timeline-card--compact': props.compact }"
  >
    <MarkdownBlock
      v-if="collapsed && compactView"
      :content="compactView.content"
      :tone="compactView.tone"
      :single-line="compactView.singleLine"
      class="timeline-muted-line"
    />

    <template v-else>
      <MarkdownBlock
        v-if="expandedFallbackView"
        :content="expandedFallbackView.content"
        :tone="expandedFallbackView.tone"
        :single-line="expandedFallbackView.singleLine"
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

      <p v-if="!expandedFallbackView && !details.length" class="timeline-muted-line">
        暂无摘要。
      </p>
    </template>
  </article>
</template>
