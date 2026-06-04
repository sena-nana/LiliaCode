import { parseInlineMarkdown, parseInlineMarkdownLines } from "./inline";
import { parseListBlock, parseListItem } from "./list";
import { isMathBlockStart, parseMathBlock, renderMathToHtml } from "./math";
import { isTableStart, parseTable } from "./table";
import { makeBlock, type FencedCodeBlock, type MarkdownBlockNode } from "./types";

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
      const list = parseListBlock(lines, index, listItem, isBlockStart);
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

function toSingleLineText(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}
