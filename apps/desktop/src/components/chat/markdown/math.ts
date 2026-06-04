import katex from "katex";
import type { MathBlock } from "./types";

const MAX_MATH_SOURCE_LENGTH = 2_000;
const MATH_RENDER_CACHE_LIMIT = 200;
const mathRenderCache = new Map<string, string | null>();

export function isMathBlockStart(line: string): boolean {
  const trimmed = line.trim();
  return trimmed.startsWith("$$") || trimmed.startsWith("\\[");
}

export function parseMathBlock(
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

export function renderMathToHtml(text: string, displayMode: boolean): string | null {
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
