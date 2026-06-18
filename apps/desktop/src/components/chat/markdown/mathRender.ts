import katex from "katex";

const MAX_MATH_SOURCE_LENGTH = 2_000;
const MATH_RENDER_CACHE_LIMIT = 200;
const mathRenderCache = new Map<string, string | null>();

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
