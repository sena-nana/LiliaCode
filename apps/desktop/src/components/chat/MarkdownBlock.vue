<script setup lang="ts">
import { computed, defineAsyncComponent, type CSSProperties } from "vue";
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
import type { ChatImageViewerSource } from "./imageViewer";
import type { MarkdownBlockTone } from "./timelineDisplay";
import { measurePerfAsync, measurePerfSync } from "@lilia/ui/diagnostics";
import type { InlineToken } from "./markdown/types";
import { createLazyLoadState } from "@lilia/ui/utils/lazyLoadState";

const markdownMathBlockLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "markdown.math-block.load",
    async () => (await import("./markdown/MarkdownMathBlock.vue")).default,
  )
);
const markdownMermaidLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "markdown.mermaid.load",
    async () => (await import("./MarkdownMermaid.vue")).default,
  )
);

const MarkdownMathBlock = defineAsyncComponent({
  suspensible: false,
  loader: () => markdownMathBlockLoad.load(),
});

const MarkdownMermaid = defineAsyncComponent({
  suspensible: false,
  loader: () => markdownMermaidLoad.load(),
});

const INLINE_CACHE_LIMIT = 200;
const BLOCK_CACHE_LIMIT = 120;
const inlineParseCache = new Map<string, InlineToken[]>();
const blockParseCache = new Map<string, MarkdownBlockNode[]>();

function readCachedInlineTokens(content: string): InlineToken[] {
  const cached = inlineParseCache.get(content);
  if (cached) return cached;
  const parsed = measurePerfSync(
    "markdown.inline.parse",
    () => parseInlineMarkdown(toSingleLineText(content)),
    { detail: `${content.length} chars` },
  );
  inlineParseCache.set(content, parsed);
  if (inlineParseCache.size > INLINE_CACHE_LIMIT) {
    const oldestKey = inlineParseCache.keys().next().value;
    if (oldestKey !== undefined) inlineParseCache.delete(oldestKey);
  }
  return parsed;
}

function readCachedBlocks(content: string): MarkdownBlockNode[] {
  const cached = blockParseCache.get(content);
  if (cached) return cached;
  const parsed = measurePerfSync(
    "markdown.block.parse",
    () => parseMarkdownBlocks(content),
    { detail: `${content.length} chars` },
  );
  blockParseCache.set(content, parsed);
  if (blockParseCache.size > BLOCK_CACHE_LIMIT) {
    const oldestKey = blockParseCache.keys().next().value;
    if (oldestKey !== undefined) blockParseCache.delete(oldestKey);
  }
  return parsed;
}

const props = withDefaults(defineProps<{
  content: string | null | undefined;
  tone?: MarkdownBlockTone;
  singleLine?: boolean;
}>(), {
  tone: "default",
  singleLine: false,
});

const normalizedContent = computed(() => normalizeMarkdownSource(props.content));
const inlineTokens = computed(() => readCachedInlineTokens(normalizedContent.value));
const blocks = computed(() => readCachedBlocks(normalizedContent.value));
const hasContent = computed(() => normalizedContent.value.length > 0);

const emit = defineEmits<{
  "open-image": [image: ChatImageViewerSource];
}>();

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
      <MarkdownInline
        :tokens="inlineTokens"
        :render-images="false"
        @open-image="emit('open-image', $event)"
      />
    </span>

    <template v-else>
      <template v-for="block in blocks" :key="block.key">
        <component
          :is="headingTag(block)"
          v-if="block.type === 'heading'"
          class="markdown-block__heading"
        >
          <MarkdownInline :tokens="block.inlines" @open-image="emit('open-image', $event)" />
        </component>

        <pre
          v-else-if="block.type === 'code'"
          class="markdown-block__code"
          :data-language="block.language || undefined"
        ><code>{{ block.text }}</code></pre>

        <MarkdownMathBlock
          v-else-if="block.type === 'math'"
          :source="block.text"
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
                  <MarkdownInline :tokens="cell" @open-image="emit('open-image', $event)" />
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
                  <MarkdownInline :tokens="cell" @open-image="emit('open-image', $event)" />
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <MarkdownList
          v-else-if="block.type === 'list' && block.list"
          :list="block.list"
          @open-image="emit('open-image', $event)"
        />

        <blockquote v-else-if="block.type === 'quote'" class="markdown-block__quote">
          <MarkdownInline :tokens="block.inlines" @open-image="emit('open-image', $event)" />
        </blockquote>

        <p v-else class="markdown-block__paragraph">
          <MarkdownInline :tokens="block.inlines" @open-image="emit('open-image', $event)" />
        </p>
      </template>
    </template>
  </div>
</template>
