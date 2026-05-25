<script setup lang="ts">
import { computed, type CSSProperties } from "vue";
import katex from "katex";
import "katex/dist/katex.min.css";
import MarkdownMermaid from "./MarkdownMermaid.vue";
import type { MarkdownBlockTone } from "./timelineDisplay";

type InlineTokenType = "text" | "code" | "strong" | "em" | "link" | "math";
type TableAlignment = "left" | "center" | "right" | null;

interface InlineToken {
  type: InlineTokenType;
  text: string;
  href: string | null;
  html: string;
}

type MarkdownBlockType =
  | "paragraph"
  | "heading"
  | "code"
  | "list"
  | "quote"
  | "table"
  | "math"
  | "mermaid";

interface MarkdownBlockNode {
  key: string;
  type: MarkdownBlockType;
  inlines: InlineToken[];
  text: string;
  html: string;
  language: string;
  ordered: boolean;
  items: InlineToken[][];
  level: 4 | 5 | 6;
  alignments: TableAlignment[];
  headers: InlineToken[][];
  rows: InlineToken[][][];
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
    html: "",
    language: "",
    ordered: false,
    items: [],
    level: 4,
    alignments: [],
    headers: [],
    rows: [],
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

      const text = codeLines.join("\n").replace(/\n+$/, "");
      const blockType = language.toLowerCase() === "mermaid" ? "mermaid" : "code";
      parsedBlocks.push(makeBlock(blockType, key, {
        text,
        language,
      }));
      continue;
    }

    const mathBlock = parseMathBlock(lines, index);
    if (mathBlock) {
      parsedBlocks.push(makeBlock("math", key, {
        text: mathBlock.text,
        html: renderMathToHtml(mathBlock.text, true),
      }));
      index = mathBlock.nextIndex;
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

    const table = parseTable(lines, index);
    if (table) {
      parsedBlocks.push(makeBlock("table", key, {
        alignments: table.alignments,
        headers: table.headers.map((cell) => parseInlineMarkdown(cell)),
        rows: table.rows.map((row) => row.map((cell) => parseInlineMarkdown(cell))),
      }));
      index = table.nextIndex;
      continue;
    }

    const paragraphLines: string[] = [];
    while (index < lines.length) {
      const paragraphLine = lines[index] ?? "";
      if (!paragraphLine.trim() || isBlockStart(paragraphLine, lines, index)) break;
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

function isBlockStart(line: string, lines?: string[], index?: number): boolean {
  return /^\s*(```+|~~~+)/.test(line) ||
    isMathBlockStart(line) ||
    (lines !== undefined && index !== undefined && isTableStart(lines, index)) ||
    /^\s*(#{1,6})\s+/.test(line) ||
    parseListItem(line) !== null ||
    /^\s*>\s?/.test(line);
}

function isMathBlockStart(line: string): boolean {
  const trimmed = line.trim();
  return trimmed.startsWith("$$") || trimmed.startsWith("\\[");
}

function parseMathBlock(
  lines: string[],
  startIndex: number,
): { text: string; nextIndex: number } | null {
  const line = lines[startIndex] ?? "";
  const trimmed = line.trim();

  if (trimmed.startsWith("$$")) {
    return parseDelimitedMathBlock(lines, startIndex, "$$", "$$");
  }

  if (trimmed.startsWith("\\[")) {
    return parseDelimitedMathBlock(lines, startIndex, "\\[", "\\]");
  }

  return null;
}

function parseDelimitedMathBlock(
  lines: string[],
  startIndex: number,
  opening: "$$" | "\\[",
  closing: "$$" | "\\]",
): { text: string; nextIndex: number } {
  const firstLine = lines[startIndex] ?? "";
  const firstTrimmed = firstLine.trim();
  const openingIndex = firstTrimmed.indexOf(opening);
  const firstRemainder = firstTrimmed.slice(openingIndex + opening.length);
  const sameLineClosingIndex = firstRemainder.lastIndexOf(closing);

  if (sameLineClosingIndex >= 0) {
    return {
      text: firstRemainder.slice(0, sameLineClosingIndex).trim(),
      nextIndex: startIndex + 1,
    };
  }

  const mathLines = firstRemainder.trimEnd() ? [firstRemainder.trimEnd()] : [];
  let index = startIndex + 1;

  while (index < lines.length) {
    const currentLine = lines[index] ?? "";
    const closingIndex = currentLine.indexOf(closing);
    if (closingIndex >= 0) {
      mathLines.push(currentLine.slice(0, closingIndex));
      index += 1;
      break;
    }

    mathLines.push(currentLine);
    index += 1;
  }

  return {
    text: mathLines.join("\n").trim(),
    nextIndex: index,
  };
}

function isTableStart(lines: string[], startIndex: number): boolean {
  return parseTable(lines, startIndex) !== null;
}

function parseTable(
  lines: string[],
  startIndex: number,
): { headers: string[]; alignments: TableAlignment[]; rows: string[][]; nextIndex: number } | null {
  const header = parseTableRow(lines[startIndex] ?? "");
  if (!header) return null;

  const alignments = parseTableDelimiter(lines[startIndex + 1] ?? "", header.length);
  if (!alignments) return null;

  const columnCount = alignments.length;
  const rows: string[][] = [];
  let index = startIndex + 2;

  while (index < lines.length) {
    const row = parseTableRow(lines[index] ?? "");
    if (!row) break;
    rows.push(normalizeTableCells(row, columnCount));
    index += 1;
  }

  return {
    headers: normalizeTableCells(header, columnCount),
    alignments,
    rows,
    nextIndex: index,
  };
}

function parseTableDelimiter(line: string, expectedColumns: number): TableAlignment[] | null {
  const cells = parseTableRow(line);
  if (!cells || cells.length !== expectedColumns) return null;

  const alignments: TableAlignment[] = [];
  for (const cell of cells) {
    const value = cell.trim();
    if (!/^:?-{3,}:?$/.test(value)) return null;
    const left = value.startsWith(":");
    const right = value.endsWith(":");
    alignments.push(left && right ? "center" : right ? "right" : left ? "left" : null);
  }
  return alignments;
}

function parseTableRow(line: string): string[] | null {
  const trimmed = line.trim();
  if (!trimmed.includes("|")) return null;

  let body = trimmed;
  if (body.startsWith("|")) body = body.slice(1);
  if (body.endsWith("|")) body = body.slice(0, -1);

  const cells: string[] = [];
  let current = "";

  for (let index = 0; index < body.length; index += 1) {
    const char = body[index] ?? "";
    if (char === "\\" && body[index + 1] === "|") {
      current += "|";
      index += 1;
      continue;
    }

    if (char === "|") {
      cells.push(current.trim());
      current = "";
      continue;
    }

    current += char;
  }

  cells.push(current.trim());
  return cells;
}

function normalizeTableCells(cells: string[], columnCount: number): string[] {
  const normalized = cells.slice(0, columnCount);
  while (normalized.length < columnCount) normalized.push("");
  return normalized;
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
  const pattern = /`([^`\n]+)`|\\\(([^\n]*?)\\\)|\$([^\s$\n](?:[^$\n]*?[^\s\\$])?)\$|\*\*([^*\n]+)\*\*|_([^_\n]+)_|\*([^*\n]+)\*|\[([^\]\n]+)\]\(([^)\s]+)\)/g;
  let lastIndex = 0;

  for (const match of text.matchAll(pattern)) {
    const index = match.index ?? 0;
    if (index > lastIndex) {
      pushTextToken(tokens, text.slice(lastIndex, index));
    }

    if (match[1] !== undefined) {
      tokens.push(makeInlineToken("code", match[1]));
    } else if (match[2] !== undefined) {
      tokens.push(makeInlineToken("math", match[2], {
        html: renderMathToHtml(match[2], false),
      }));
    } else if (match[3] !== undefined) {
      tokens.push(makeInlineToken("math", match[3], {
        html: renderMathToHtml(match[3], false),
      }));
    } else if (match[4] !== undefined) {
      tokens.push(makeInlineToken("strong", match[4]));
    } else if (match[5] !== undefined || match[6] !== undefined) {
      tokens.push(makeInlineToken("em", match[5] ?? match[6] ?? ""));
    } else if (match[7] !== undefined && match[8] !== undefined) {
      const href = normalizeHref(match[8]);
      tokens.push({
        type: href ? "link" : "text",
        text: match[7],
        href,
        html: "",
      });
    }

    lastIndex = index + match[0].length;
  }

  if (lastIndex < text.length) {
    pushTextToken(tokens, text.slice(lastIndex));
  }

  return tokens;
}

function makeInlineToken(
  type: InlineTokenType,
  text: string,
  overrides: Partial<Omit<InlineToken, "type" | "text">> = {},
): InlineToken {
  return {
    type,
    text,
    href: null,
    html: "",
    ...overrides,
  };
}

function pushTextToken(tokens: InlineToken[], text: string) {
  if (text) tokens.push(makeInlineToken("text", text));
}

function renderMathToHtml(text: string, displayMode: boolean): string {
  return katex.renderToString(text, {
    displayMode,
    throwOnError: false,
    trust: false,
    strict: "ignore",
  });
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
      <template v-for="(token, index) in inlineTokens" :key="`${token.type}:${index}`">
        <code v-if="token.type === 'code'">{{ token.text }}</code>
        <span
          v-else-if="token.type === 'math'"
          class="markdown-block__math-inline"
          v-html="token.html"
        />
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
            <span
              v-else-if="token.type === 'math'"
              class="markdown-block__math-inline"
              v-html="token.html"
            />
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

        <div v-else-if="block.type === 'table'" class="markdown-block__table-wrap">
          <table class="markdown-block__table">
            <thead>
              <tr>
                <th
                  v-for="(cell, cellIndex) in block.headers"
                  :key="`head:${cellIndex}`"
                  :style="tableAlignmentStyle(block.alignments[cellIndex] ?? null)"
                >
                  <template v-for="(token, index) in cell" :key="`${token.type}:${index}`">
                    <code v-if="token.type === 'code'">{{ token.text }}</code>
                    <span
                      v-else-if="token.type === 'math'"
                      class="markdown-block__math-inline"
                      v-html="token.html"
                    />
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
                  <template v-for="(token, index) in cell" :key="`${token.type}:${index}`">
                    <code v-if="token.type === 'code'">{{ token.text }}</code>
                    <span
                      v-else-if="token.type === 'math'"
                      class="markdown-block__math-inline"
                      v-html="token.html"
                    />
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
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <component
          :is="block.ordered ? 'ol' : 'ul'"
          v-else-if="block.type === 'list'"
          class="markdown-block__list"
        >
          <li v-for="(item, itemIndex) in block.items" :key="itemIndex">
            <template v-for="(token, index) in item" :key="`${token.type}:${index}`">
              <code v-if="token.type === 'code'">{{ token.text }}</code>
              <span
                v-else-if="token.type === 'math'"
                class="markdown-block__math-inline"
                v-html="token.html"
              />
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
            <span
              v-else-if="token.type === 'math'"
              class="markdown-block__math-inline"
              v-html="token.html"
            />
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
            <span
              v-else-if="token.type === 'math'"
              class="markdown-block__math-inline"
              v-html="token.html"
            />
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
