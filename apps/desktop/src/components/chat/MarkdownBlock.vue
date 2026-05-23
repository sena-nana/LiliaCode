<script setup lang="ts">
import { computed } from "vue";
import type { MarkdownBlockTone } from "./timelineDisplay";

type InlineTokenType = "text" | "code" | "strong" | "em" | "link";

interface InlineToken {
  type: InlineTokenType;
  text: string;
  href: string | null;
}

type MarkdownBlockType = "paragraph" | "heading" | "code" | "list" | "quote";

interface MarkdownBlockNode {
  key: string;
  type: MarkdownBlockType;
  inlines: InlineToken[];
  text: string;
  language: string;
  ordered: boolean;
  items: InlineToken[][];
  level: 4 | 5 | 6;
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

const inlineTokens = computed(() =>
  parseInlineMarkdown(toSingleLineText(normalizedContent.value)),
);

const blocks = computed(() => parseMarkdownBlocks(normalizedContent.value));

const hasContent = computed(() => normalizedContent.value.length > 0);

function normalizeMarkdownSource(content: string | null | undefined): string {
  return (content ?? "").replace(/\r\n?/g, "\n").trim();
}

function toSingleLineText(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

function makeBlock(
  type: MarkdownBlockType,
  key: string,
  overrides: Partial<Omit<MarkdownBlockNode, "type" | "key">> = {},
): MarkdownBlockNode {
  return {
    key,
    type,
    inlines: [],
    text: "",
    language: "",
    ordered: false,
    items: [],
    level: 4,
    ...overrides,
  };
}

function parseMarkdownBlocks(source: string): MarkdownBlockNode[] {
  if (!source) return [];

  const lines = source.split("\n");
  const parsedBlocks: MarkdownBlockNode[] = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index] ?? "";
    const trimmed = line.trim();
    const key = `block-${parsedBlocks.length}`;

    if (!trimmed) {
      index += 1;
      continue;
    }

    const fence = line.match(/^\s*(```+|~~~+)\s*([A-Za-z0-9_-]*)?.*$/);
    if (fence) {
      const fenceMarker = fence[1] ?? "```";
      const closingFence = fenceMarker[0]?.repeat(fenceMarker.length) ?? "```";
      const language = fence[2] ?? "";
      const codeLines: string[] = [];
      index += 1;

      while (index < lines.length) {
        const codeLine = lines[index] ?? "";
        if (codeLine.trimStart().startsWith(closingFence)) {
          index += 1;
          break;
        }
        codeLines.push(codeLine);
        index += 1;
      }

      parsedBlocks.push(makeBlock("code", key, {
        text: codeLines.join("\n").replace(/\n+$/, ""),
        language,
      }));
      continue;
    }

    const heading = line.match(/^\s*(#{1,6})\s+(.+)$/);
    if (heading) {
      const level = Math.min(6, Math.max(4, (heading[1]?.length ?? 1) + 3)) as 4 | 5 | 6;
      const text = (heading[2] ?? "").trim();
      parsedBlocks.push(makeBlock("heading", key, {
        inlines: parseInlineMarkdown(text),
        level,
      }));
      index += 1;
      continue;
    }

    const listItem = parseListItem(line);
    if (listItem) {
      const ordered = listItem.ordered;
      const items: InlineToken[][] = [];

      while (index < lines.length) {
        const item = parseListItem(lines[index] ?? "");
        if (!item || item.ordered !== ordered) break;
        items.push(parseInlineMarkdown(item.text));
        index += 1;
      }

      parsedBlocks.push(makeBlock("list", key, { ordered, items }));
      continue;
    }

    if (/^\s*>\s?/.test(line)) {
      const quoteLines: string[] = [];

      while (index < lines.length && /^\s*>\s?/.test(lines[index] ?? "")) {
        quoteLines.push((lines[index] ?? "").replace(/^\s*>\s?/, "").trim());
        index += 1;
      }

      const text = quoteLines.join(" ").trim();
      parsedBlocks.push(makeBlock("quote", key, {
        inlines: parseInlineMarkdown(text),
      }));
      continue;
    }

    const paragraphLines: string[] = [];
    while (index < lines.length) {
      const paragraphLine = lines[index] ?? "";
      if (!paragraphLine.trim() || isBlockStart(paragraphLine)) break;
      paragraphLines.push(paragraphLine.trim());
      index += 1;
    }

    const text = paragraphLines.join(" ").trim();
    if (text) {
      parsedBlocks.push(makeBlock("paragraph", key, {
        inlines: parseInlineMarkdown(text),
      }));
    }
  }

  return parsedBlocks;
}

function isBlockStart(line: string): boolean {
  return /^\s*(```+|~~~+)/.test(line) ||
    /^\s*(#{1,6})\s+/.test(line) ||
    parseListItem(line) !== null ||
    /^\s*>\s?/.test(line);
}

function parseListItem(line: string): { ordered: boolean; text: string } | null {
  const match = line.match(/^\s*(?:(\d+)[.)]|[-*+])\s+(.+)$/);
  if (!match) return null;
  return {
    ordered: match[1] !== undefined,
    text: (match[2] ?? "").trim(),
  };
}

function parseInlineMarkdown(text: string): InlineToken[] {
  if (!text) return [];

  const tokens: InlineToken[] = [];
  const pattern = /`([^`\n]+)`|\*\*([^*\n]+)\*\*|_([^_\n]+)_|\*([^*\n]+)\*|\[([^\]\n]+)\]\(([^)\s]+)\)/g;
  let lastIndex = 0;

  for (const match of text.matchAll(pattern)) {
    const index = match.index ?? 0;
    if (index > lastIndex) {
      pushTextToken(tokens, text.slice(lastIndex, index));
    }

    if (match[1] !== undefined) {
      tokens.push({ type: "code", text: match[1], href: null });
    } else if (match[2] !== undefined) {
      tokens.push({ type: "strong", text: match[2], href: null });
    } else if (match[3] !== undefined || match[4] !== undefined) {
      tokens.push({ type: "em", text: match[3] ?? match[4] ?? "", href: null });
    } else if (match[5] !== undefined && match[6] !== undefined) {
      const href = normalizeHref(match[6]);
      tokens.push({
        type: href ? "link" : "text",
        text: match[5],
        href,
      });
    }

    lastIndex = index + match[0].length;
  }

  if (lastIndex < text.length) {
    pushTextToken(tokens, text.slice(lastIndex));
  }

  return tokens;
}

function pushTextToken(tokens: InlineToken[], text: string) {
  if (text) tokens.push({ type: "text", text, href: null });
}

function normalizeHref(href: string): string | null {
  const trimmed = href.trim();
  if (!trimmed) return null;
  if (/^(https?:|mailto:)/i.test(trimmed)) return trimmed;
  if (/^(#|\/|\.\/|\.\.\/)/.test(trimmed)) return trimmed;
  return null;
}

function linkTarget(href: string | null): string | undefined {
  return href && /^https?:/i.test(href) ? "_blank" : undefined;
}

function headingTag(block: MarkdownBlockNode): "h4" | "h5" | "h6" {
  return `h${block.level}` as "h4" | "h5" | "h6";
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
      <template v-for="(token, index) in inlineTokens" :key="`${token.type}:${index}`">
        <code v-if="token.type === 'code'">{{ token.text }}</code>
        <strong v-else-if="token.type === 'strong'">{{ token.text }}</strong>
        <em v-else-if="token.type === 'em'">{{ token.text }}</em>
        <a
          v-else-if="token.type === 'link' && token.href"
          :href="token.href"
          :target="linkTarget(token.href)"
          rel="noreferrer"
        >{{ token.text }}</a>
        <template v-else>{{ token.text }}</template>
      </template>
    </span>

    <template v-else>
      <template v-for="block in blocks" :key="block.key">
        <component
          :is="headingTag(block)"
          v-if="block.type === 'heading'"
          class="markdown-block__heading"
        >
          <template v-for="(token, index) in block.inlines" :key="`${token.type}:${index}`">
            <code v-if="token.type === 'code'">{{ token.text }}</code>
            <strong v-else-if="token.type === 'strong'">{{ token.text }}</strong>
            <em v-else-if="token.type === 'em'">{{ token.text }}</em>
            <a
              v-else-if="token.type === 'link' && token.href"
              :href="token.href"
              :target="linkTarget(token.href)"
              rel="noreferrer"
            >{{ token.text }}</a>
            <template v-else>{{ token.text }}</template>
          </template>
        </component>

        <pre
          v-else-if="block.type === 'code'"
          class="markdown-block__code"
          :data-language="block.language || undefined"
        ><code>{{ block.text }}</code></pre>

        <component
          :is="block.ordered ? 'ol' : 'ul'"
          v-else-if="block.type === 'list'"
          class="markdown-block__list"
        >
          <li v-for="(item, itemIndex) in block.items" :key="itemIndex">
            <template v-for="(token, index) in item" :key="`${token.type}:${index}`">
              <code v-if="token.type === 'code'">{{ token.text }}</code>
              <strong v-else-if="token.type === 'strong'">{{ token.text }}</strong>
              <em v-else-if="token.type === 'em'">{{ token.text }}</em>
              <a
                v-else-if="token.type === 'link' && token.href"
                :href="token.href"
                :target="linkTarget(token.href)"
                rel="noreferrer"
              >{{ token.text }}</a>
              <template v-else>{{ token.text }}</template>
            </template>
          </li>
        </component>

        <blockquote v-else-if="block.type === 'quote'" class="markdown-block__quote">
          <template v-for="(token, index) in block.inlines" :key="`${token.type}:${index}`">
            <code v-if="token.type === 'code'">{{ token.text }}</code>
            <strong v-else-if="token.type === 'strong'">{{ token.text }}</strong>
            <em v-else-if="token.type === 'em'">{{ token.text }}</em>
            <a
              v-else-if="token.type === 'link' && token.href"
              :href="token.href"
              :target="linkTarget(token.href)"
              rel="noreferrer"
            >{{ token.text }}</a>
            <template v-else>{{ token.text }}</template>
          </template>
        </blockquote>

        <p v-else class="markdown-block__paragraph">
          <template v-for="(token, index) in block.inlines" :key="`${token.type}:${index}`">
            <code v-if="token.type === 'code'">{{ token.text }}</code>
            <strong v-else-if="token.type === 'strong'">{{ token.text }}</strong>
            <em v-else-if="token.type === 'em'">{{ token.text }}</em>
            <a
              v-else-if="token.type === 'link' && token.href"
              :href="token.href"
              :target="linkTarget(token.href)"
              rel="noreferrer"
            >{{ token.text }}</a>
            <template v-else>{{ token.text }}</template>
          </template>
        </p>
      </template>
    </template>
  </div>
</template>
