<script setup lang="ts">
import { computed, type CSSProperties } from "vue";
import "katex/dist/katex.min.css";
import MarkdownMermaid from "./MarkdownMermaid.vue";
import MarkdownInline from "./markdown/MarkdownInline.vue";
import MarkdownList from "./markdown/MarkdownList.vue";
import {
  normalizeMarkdownSource,
  parseInlineMarkdown,
  parseMarkdownBlocks,
  toSingleLineText,
  type MarkdownBlockNode,
  type TableAlignment,
} from "./markdown/markdownParser";
import type { MarkdownBlockTone } from "./timelineDisplay";

const props = withDefaults(defineProps<{
  content: string | null | undefined;
  tone?: MarkdownBlockTone;
  singleLine?: boolean;
}>(), {
  tone: "default",
  singleLine: false,
});

const normalizedContent = computed(() => normalizeMarkdownSource(props.content));
const inlineTokens = computed(() => parseInlineMarkdown(toSingleLineText(normalizedContent.value)));
const blocks = computed(() => parseMarkdownBlocks(normalizedContent.value));
const hasContent = computed(() => normalizedContent.value.length > 0);

function headingTag(block: MarkdownBlockNode): "h4" | "h5" | "h6" {
  return `h${block.level}` as "h4" | "h5" | "h6";
}

function tableAlignmentStyle(alignment: TableAlignment): CSSProperties | undefined {
  return alignment ? { textAlign: alignment } : undefined;
}
</script>

<template>
  <div
    v-if="hasContent"
    class="markdown-block"
    :class="[
      `markdown-block--${tone}`,
      { 'markdown-block--single-line': singleLine },
    ]"
  >
    <span v-if="singleLine" class="markdown-block__line">
      <MarkdownInline :tokens="inlineTokens" :render-images="false" />
    </span>

    <template v-else>
      <template v-for="block in blocks" :key="block.key">
        <component
          :is="headingTag(block)"
          v-if="block.type === 'heading'"
          class="markdown-block__heading"
        >
          <MarkdownInline :tokens="block.inlines" />
        </component>

        <pre
          v-else-if="block.type === 'code'"
          class="markdown-block__code"
          :data-language="block.language || undefined"
        ><code>{{ block.text }}</code></pre>

        <div
          v-else-if="block.type === 'math'"
          class="markdown-block__math-block"
          v-html="block.html"
        />

        <MarkdownMermaid
          v-else-if="block.type === 'mermaid'"
          :block-key="block.key"
          :source="block.text"
        />

        <hr
          v-else-if="block.type === 'divider'"
          class="markdown-block__divider"
          aria-hidden="true"
        >

        <div v-else-if="block.type === 'table'" class="markdown-block__table-wrap">
          <table class="markdown-block__table">
            <thead>
              <tr>
                <th
                  v-for="(cell, cellIndex) in block.headers"
                  :key="`head:${cellIndex}`"
                  :style="tableAlignmentStyle(block.alignments[cellIndex] ?? null)"
                >
                  <MarkdownInline :tokens="cell" />
                </th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, rowIndex) in block.rows" :key="`row:${rowIndex}`">
                <td
                  v-for="(cell, cellIndex) in row"
                  :key="`cell:${rowIndex}:${cellIndex}`"
                  :style="tableAlignmentStyle(block.alignments[cellIndex] ?? null)"
                >
                  <MarkdownInline :tokens="cell" />
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <MarkdownList
          v-else-if="block.type === 'list' && block.list"
          :list="block.list"
        />

        <blockquote v-else-if="block.type === 'quote'" class="markdown-block__quote">
          <MarkdownInline :tokens="block.inlines" />
        </blockquote>

        <p v-else class="markdown-block__paragraph">
          <MarkdownInline :tokens="block.inlines" />
        </p>
      </template>
    </template>
  </div>
</template>
