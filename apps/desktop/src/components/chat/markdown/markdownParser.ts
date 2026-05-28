import katex from "katex";

export type InlineTokenType =
  | "text"
  | "code"
  | "strong"
  | "em"
  | "link"
  | "image"
  | "math"
  | "delete"
  | "break";

export type TableAlignment = "left" | "center" | "right" | null;

export interface InlineToken {
  type: InlineTokenType;
  text: string;
  href: string | null;
  html: string;
}

export type MarkdownBlockType =
  | "paragraph"
  | "heading"
  | "code"
  | "list"
  | "quote"
  | "table"
  | "math"
  | "divider"
  | "mermaid";

export interface MarkdownListNode {
  ordered: boolean;
  start: number | null;
  items: MarkdownListItem[];
}

export interface MarkdownListItem {
  inlines: InlineToken[];
  taskChecked: boolean | null;
  children: MarkdownListNode[];
}

export interface MarkdownBlockNode {
  key: string;
  type: MarkdownBlockType;
  inlines: InlineToken[];
  text: string;
  html: string;
  language: string;
  list: MarkdownListNode | null;
  level: 4 | 5 | 6;
  alignments: TableAlignment[];
  headers: InlineToken[][];
  rows: InlineToken[][][];
}

interface DraftMarkdownListNode {
  ordered: boolean;
  start: number | null;
  items: DraftMarkdownListItem[];
}

interface DraftMarkdownListItem {
  lines: string[];
  taskChecked: boolean | null;
  children: DraftMarkdownListNode[];
}

interface FencedCodeBlock {
  text: string;
  language: string;
  nextIndex: number;
  closed: boolean;
}

interface MathBlock {
  text: string;
  raw: string;
  nextIndex: number;
  closed: boolean;
}

interface ParsedListItem {
  indent: number;
  ordered: boolean;
  number: number | null;
  text: string;
  taskChecked: boolean | null;
}

const MAX_MATH_SOURCE_LENGTH = 2_000;
const MATH_RENDER_CACHE_LIMIT = 200;
const mathRenderCache = new Map<string, string | null>();
const INLINE_TOKEN_PATTERN = /`(?<code>[^`\n]+)`|!\[(?<imageAlt>[^\]\n]*)\]\((?<imageSrc>[^)\s]+)\)|\\\((?<parenMath>[^\n]*?)\\\)|~~(?<delete>[^~\n]+)~~|\*\*(?<starStrong>[^*\n]+)\*\*|__(?<underscoreStrong>[^_\n]+)__|_(?<underscoreEm>[^_\n]+)_|\*(?<starEm>[^*\n]+)\*|\[(?<linkText>[^\]\n]+)\]\((?<linkHref>[^)\s]+)\)|<(?<angleHref>(?:https?:\/\/|mailto:)[^<>\s]+)>/g;

export function normalizeMarkdownSource(content: string | null | undefined): string {
  return (content ?? "").replace(/\r\n?/g, "\n").trim();
}

export function toSingleLineText(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

export function parseMarkdownBlocks(source: string): MarkdownBlockNode[] {
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

    const fence = parseFencedCodeBlock(lines, index);
    if (fence) {
      const blockType = fence.closed && fence.language.toLowerCase() === "mermaid"
        ? "mermaid"
        : "code";
      parsedBlocks.push(makeBlock(blockType, key, {
        text: fence.text,
        language: fence.language,
      }));
      index = fence.nextIndex;
      continue;
    }

    const mathBlock = parseMathBlock(lines, index);
    if (mathBlock) {
      const html = mathBlock.closed ? renderMathToHtml(mathBlock.text, true) : null;
      if (html) {
        parsedBlocks.push(makeBlock("math", key, {
          text: mathBlock.text,
          html,
        }));
      } else {
        parsedBlocks.push(makeBlock("paragraph", key, {
          inlines: parseInlineMarkdown(toSingleLineText(mathBlock.raw)),
        }));
      }
      index = mathBlock.nextIndex;
      continue;
    }

    if (isThematicBreak(line)) {
      parsedBlocks.push(makeBlock("divider", key));
      index += 1;
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
      const list = parseListBlock(lines, index, listItem);
      parsedBlocks.push(makeBlock("list", key, { list: list.node }));
      index = list.nextIndex;
      continue;
    }

    if (/^\s*>\s?/.test(line)) {
      const quoteLines: string[] = [];

      while (index < lines.length && /^\s*>\s?/.test(lines[index] ?? "")) {
        quoteLines.push((lines[index] ?? "").replace(/^\s*>\s?/, "").trim());
        index += 1;
      }

      parsedBlocks.push(makeBlock("quote", key, {
        inlines: parseInlineMarkdown(quoteLines.join(" ").trim()),
      }));
      continue;
    }

    const table = parseTable(lines, index);
    if (table) {
      parsedBlocks.push(makeBlock("table", key, {
        alignments: table.alignments,
        headers: table.headers.map(parseInlineMarkdown),
        rows: table.rows.map((row) => row.map(parseInlineMarkdown)),
      }));
      index = table.nextIndex;
      continue;
    }

    const paragraphLines: string[] = [];
    while (index < lines.length) {
      const paragraphLine = lines[index] ?? "";
      if (!paragraphLine.trim() || isIndentedBlockStart(paragraphLine, lines, index)) break;
      paragraphLines.push(paragraphLine);
      index += 1;
    }

    if (paragraphLines.length > 0) {
      parsedBlocks.push(makeBlock("paragraph", key, {
        inlines: parseInlineMarkdownLines(paragraphLines),
      }));
    }
  }

  return parsedBlocks;
}

export function parseInlineMarkdown(text: string): InlineToken[] {
  if (!text) return [];

  const tokens: InlineToken[] = [];
  INLINE_TOKEN_PATTERN.lastIndex = 0;
  let lastIndex = 0;

  for (const match of text.matchAll(INLINE_TOKEN_PATTERN)) {
    const index = match.index ?? 0;
    if (index > lastIndex) pushTextToken(tokens, text.slice(lastIndex, index));
    pushInlineMatch(tokens, match);
    lastIndex = index + match[0].length;
  }

  if (lastIndex < text.length) pushTextToken(tokens, text.slice(lastIndex));
  return tokens;
}

function parseInlineMarkdownLines(lines: string[]): InlineToken[] {
  const tokens: InlineToken[] = [];
  lines.forEach((line, index) => {
    const hardBreak = /(?:\\| {2,})$/.test(line);
    const text = line.replace(/(?:\\| {2,})$/, "");
    tokens.push(...parseInlineMarkdown(index === 0 ? text.trimStart() : text.trim()));
    if (index < lines.length - 1) {
      if (hardBreak) {
        tokens.push(makeInlineToken("break", ""));
      } else {
        pushTextToken(tokens, " ");
      }
    }
  });
  return tokens;
}

function pushInlineMatch(tokens: InlineToken[], match: RegExpMatchArray) {
  const groups = match.groups ?? {};

  if (groups.code !== undefined) {
    tokens.push(makeInlineToken("code", groups.code));
    return;
  }

  if (groups.imageAlt !== undefined && groups.imageSrc !== undefined) {
    pushImageToken(tokens, groups.imageAlt, groups.imageSrc, match[0]);
    return;
  }

  if (groups.parenMath !== undefined) {
    const html = renderMathToHtml(groups.parenMath, false);
    if (html) {
      tokens.push(makeInlineToken("math", groups.parenMath, { html }));
    } else {
      pushTextToken(tokens, match[0]);
    }
    return;
  }

  if (groups.delete !== undefined) {
    tokens.push(makeInlineToken("delete", groups.delete));
    return;
  }

  const strong = groups.starStrong ?? groups.underscoreStrong;
  if (strong !== undefined) {
    tokens.push(makeInlineToken("strong", strong));
    return;
  }

  const emphasis = groups.underscoreEm ?? groups.starEm;
  if (emphasis !== undefined) {
    tokens.push(makeInlineToken("em", emphasis));
    return;
  }

  if (groups.linkText !== undefined && groups.linkHref !== undefined) {
    pushExplicitLinkToken(tokens, groups.linkText, groups.linkHref);
    return;
  }

  if (groups.angleHref !== undefined) {
    pushExplicitLinkToken(tokens, groups.angleHref, groups.angleHref);
  }
}

function pushImageToken(tokens: InlineToken[], alt: string, rawSrc: string, rawText: string) {
  const src = normalizeImageSrc(rawSrc);
  tokens.push(src
    ? { type: "image", text: alt, href: src, html: "" }
    : makeInlineToken("text", rawText));
}

function pushExplicitLinkToken(tokens: InlineToken[], text: string, rawHref: string) {
  const href = normalizeHref(rawHref);
  tokens.push(href
    ? { type: "link", text, href, html: "" }
    : makeInlineToken("text", text));
}

function pushTextToken(tokens: InlineToken[], text: string) {
  if (!text) return;

  const linkPattern = /\bhttps?:\/\/[^\s<]+|\bmailto:[^\s<]+/g;
  let lastIndex = 0;
  for (const match of text.matchAll(linkPattern)) {
    const index = match.index ?? 0;
    if (index > lastIndex) {
      tokens.push(makeInlineToken("text", text.slice(lastIndex, index)));
    }

    const raw = match[0];
    const { href, suffix } = splitAutoLink(raw);
    const normalizedHref = normalizeHref(href);
    tokens.push(normalizedHref
      ? { type: "link", text: href, href: normalizedHref, html: "" }
      : makeInlineToken("text", href));
    if (suffix) tokens.push(makeInlineToken("text", suffix));
    lastIndex = index + raw.length;
  }

  if (lastIndex < text.length) {
    tokens.push(makeInlineToken("text", text.slice(lastIndex)));
  }
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
    list: null,
    level: 4,
    alignments: [],
    headers: [],
    rows: [],
    ...overrides,
  };
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

function parseFencedCodeBlock(lines: string[], startIndex: number): FencedCodeBlock | null {
  const line = lines[startIndex] ?? "";
  const fence = line.match(/^\s*(```+|~~~+)\s*([A-Za-z0-9_-]*)?.*$/);
  if (!fence) return null;

  const fenceMarker = fence[1] ?? "```";
  const closingFence = fenceMarker[0]?.repeat(fenceMarker.length) ?? "```";
  const language = fence[2] ?? "";
  const codeLines: string[] = [];
  let index = startIndex + 1;
  let closed = false;

  while (index < lines.length) {
    const codeLine = lines[index] ?? "";
    if (codeLine.trimStart().startsWith(closingFence)) {
      closed = true;
      index += 1;
      break;
    }
    codeLines.push(codeLine);
    index += 1;
  }

  return {
    text: codeLines.join("\n").replace(/\n+$/, ""),
    language,
    nextIndex: index,
    closed,
  };
}

function isBlockStart(line: string, lines?: string[], index?: number): boolean {
  return /^\s*(```+|~~~+)/.test(line) ||
    isMathBlockStart(line) ||
    isThematicBreak(line) ||
    (lines !== undefined && index !== undefined && isTableStart(lines, index)) ||
    /^\s*(#{1,6})\s+/.test(line) ||
    parseListItem(line) !== null ||
    /^\s*>\s?/.test(line);
}

function isIndentedBlockStart(line: string, lines?: string[], index?: number): boolean {
  const item = parseListItem(line);
  if (!item) return isBlockStart(line, lines, index);
  return item.indent <= 3;
}

function isThematicBreak(line: string): boolean {
  return /^ {0,3}([-*_])(?:\s*\1){2,}\s*$/.test(line);
}

function isMathBlockStart(line: string): boolean {
  const trimmed = line.trim();
  return trimmed.startsWith("$$") || trimmed.startsWith("\\[");
}

function parseMathBlock(
  lines: string[],
  startIndex: number,
): MathBlock | null {
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
): MathBlock {
  const firstLine = lines[startIndex] ?? "";
  const firstTrimmed = firstLine.trim();
  const openingIndex = firstTrimmed.indexOf(opening);
  const firstRemainder = firstTrimmed.slice(openingIndex + opening.length);
  const sameLineClosingIndex = firstRemainder.lastIndexOf(closing);

  if (sameLineClosingIndex >= 0) {
    return {
      text: firstRemainder.slice(0, sameLineClosingIndex).trim(),
      raw: firstLine,
      nextIndex: startIndex + 1,
      closed: true,
    };
  }

  const mathLines = firstRemainder.trimEnd() ? [firstRemainder.trimEnd()] : [];
  const rawLines = [firstLine];
  let index = startIndex + 1;

  while (index < lines.length) {
    const currentLine = lines[index] ?? "";
    rawLines.push(currentLine);
    const closingIndex = currentLine.indexOf(closing);
    if (closingIndex >= 0) {
      mathLines.push(currentLine.slice(0, closingIndex));
      index += 1;
      return {
        text: mathLines.join("\n").trim(),
        raw: rawLines.join("\n"),
        nextIndex: index,
        closed: true,
      };
    }

    mathLines.push(currentLine);
    index += 1;
  }

  return {
    text: mathLines.join("\n").trim(),
    raw: rawLines.join("\n"),
    nextIndex: index,
    closed: false,
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

function parseListBlock(
  lines: string[],
  startIndex: number,
  first: ParsedListItem,
): { node: MarkdownListNode; nextIndex: number } {
  const root = makeDraftList(first.ordered, first.number);
  const stack: Array<{ indent: number; node: DraftMarkdownListNode }> = [
    { indent: first.indent, node: root },
  ];
  let lastItem: DraftMarkdownListItem | null = null;
  let index = startIndex;

  while (index < lines.length) {
    const line = lines[index] ?? "";
    if (!line.trim()) break;

    const item = index === startIndex ? first : parseListItem(line);
    if (item) {
      if (item.indent < first.indent) break;

      while (stack.length > 1 && item.indent < stack[stack.length - 1]!.indent) {
        stack.pop();
      }

      let current = stack[stack.length - 1]!;
      if (item.indent > current.indent) {
        if (!lastItem) break;
        const child = makeDraftList(item.ordered, item.number);
        lastItem.children.push(child);
        current = { indent: item.indent, node: child };
        stack.push(current);
      } else if (item.ordered !== current.node.ordered && current.node.items.length > 0) {
        break;
      } else {
        current.node.ordered = item.ordered;
      }

      lastItem = {
        lines: [item.text],
        taskChecked: item.taskChecked,
        children: [],
      };
      current.node.items.push(lastItem);
      index += 1;
      continue;
    }

    const indent = leadingSpaceCount(line);
    if (!lastItem || indent <= first.indent || isBlockStart(line, lines, index)) break;
    lastItem.lines.push(line.slice(Math.min(indent, first.indent + 2)).trim());
    index += 1;
  }

  return {
    node: finalizeListNode(root),
    nextIndex: index,
  };
}

function makeDraftList(ordered: boolean, start: number | null = null): DraftMarkdownListNode {
  return {
    ordered,
    start: ordered ? start : null,
    items: [],
  };
}

function finalizeListNode(node: DraftMarkdownListNode): MarkdownListNode {
  return {
    ordered: node.ordered,
    start: node.start,
    items: node.items.map((item) => ({
      inlines: parseInlineMarkdownLines(item.lines),
      taskChecked: item.taskChecked,
      children: item.children.map(finalizeListNode),
    })),
  };
}

function leadingSpaceCount(line: string): number {
  return line.match(/^ */)?.[0].length ?? 0;
}

function parseListItem(line: string): ParsedListItem | null {
  const match = line.match(/^( *)(?:(\d+)[.)]|([-*+]))\s+(.+)$/);
  if (!match) return null;
  const text = (match[4] ?? "").trim();
  const task = text.match(/^\[([ xX])\]\s+(.*)$/);
  return {
    indent: match[1]?.length ?? 0,
    ordered: match[2] !== undefined,
    number: match[2] !== undefined ? Number.parseInt(match[2], 10) : null,
    text: task ? (task[2] ?? "").trim() : text,
    taskChecked: task ? (task[1] ?? "").toLowerCase() === "x" : null,
  };
}

function splitAutoLink(raw: string): { href: string; suffix: string } {
  let href = raw;
  let suffix = "";
  while (/[.,!?;:]$/.test(href) || (href.endsWith(")") && !hasBalancedParens(href))) {
    suffix = href.slice(-1) + suffix;
    href = href.slice(0, -1);
  }
  return { href, suffix };
}

function hasBalancedParens(text: string): boolean {
  const opens = (text.match(/\(/g) ?? []).length;
  const closes = (text.match(/\)/g) ?? []).length;
  return closes <= opens;
}

function renderMathToHtml(text: string, displayMode: boolean): string | null {
  const source = text.trim();
  if (!source || source.length > MAX_MATH_SOURCE_LENGTH) return null;

  const cacheKey = `${displayMode ? "block" : "inline"}:${source}`;
  if (mathRenderCache.has(cacheKey)) {
    return mathRenderCache.get(cacheKey) ?? null;
  }

  let html: string | null = null;
  try {
    html = katex.renderToString(source, {
      displayMode,
      throwOnError: false,
      trust: false,
      strict: "ignore",
    });
  } catch {
    html = null;
  }

  mathRenderCache.set(cacheKey, html);
  if (mathRenderCache.size > MATH_RENDER_CACHE_LIMIT) {
    const oldestKey = mathRenderCache.keys().next().value;
    if (oldestKey !== undefined) mathRenderCache.delete(oldestKey);
  }

  return html;
}

function normalizeHref(href: string): string | null {
  const trimmed = href.trim();
  if (!trimmed) return null;
  if (/^(https?:|mailto:)/i.test(trimmed)) return trimmed;
  if (/^(#|\/|\.\/|\.\.\/)/.test(trimmed)) return trimmed;
  return null;
}

function normalizeImageSrc(src: string): string | null {
  const trimmed = src.trim();
  if (!trimmed || /[\s\u0000-\u001F]/.test(trimmed)) return null;
  if (/^https?:\/\//i.test(trimmed)) return trimmed;
  if (/^(asset:|tauri:\/\/)/i.test(trimmed)) return trimmed;
  if (/^(\/|\.\/|\.\.\/)/.test(trimmed)) return trimmed;
  if (/^[A-Za-z]:[\\/]/.test(trimmed)) return trimmed.replace(/\\/g, "/");
  if (/^[A-Za-z0-9._~!$&'()*+,;=:@%/-]+$/.test(trimmed) && !trimmed.includes(":")) {
    return trimmed;
  }
  return null;
}
