import { renderMathToHtml } from "./math";
import { makeInlineToken, type InlineToken } from "./types";

const INLINE_TOKEN_PATTERN = /`(?<code>[^`\n]+)`|!\[(?<imageAlt>[^\]\n]*)\]\((?<imageSrc>[^)\s]+)\)|\\\((?<parenMath>[^\n]*?)\\\)|~~(?<delete>[^~\n]+)~~|\*\*(?<starStrong>[^*\n]+)\*\*|__(?<underscoreStrong>[^_\n]+)__|_(?<underscoreEm>[^_\n]+)_|\*(?<starEm>[^*\n]+)\*|\[(?<linkText>[^\]\n]+)\]\((?<linkHref>[^)\s]+)\)|<(?<angleHref>(?:https?:\/\/|mailto:)[^<>\s]+)>/g;

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

export function parseInlineMarkdownLines(lines: string[]): InlineToken[] {
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
