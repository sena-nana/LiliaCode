import type { ChatContextSearchResult } from "@lilia/contracts";
import type { MentionRange } from "./composerParts";

export function isAbsolutePathLike(value: string): boolean {
  const trimmed = value.trim();
  return /^[a-zA-Z]:[\\/]/.test(trimmed) ||
    trimmed.startsWith("\\\\") ||
    trimmed.startsWith("/");
}

export function readContextMentionRange(text: string, cursor: number): MentionRange | null {
  const end = Math.min(Math.max(cursor, 0), text.length);
  const prefix = text.slice(0, end);
  const start = prefix.lastIndexOf("@");
  if (start < 0) return null;
  const query = text.slice(start + 1, end);
  if (query.length > 240 || /[\n\r]/.test(query)) return null;
  if (/\s/.test(query) && !isAbsolutePathLike(query)) return null;
  return { start, end, query };
}

export function isContextPathQueryLike(value: string): boolean {
  return value.includes("/") || value.includes("\\");
}

export function compactPathLabel(value: string): string {
  return value.replace(/\\/g, "/");
}

export function contextInlinePath(result: ChatContextSearchResult): string | null {
  const path = compactPathLabel(result.relativePath);
  return path && path !== result.attachment.name ? path : null;
}

export function readConversationReferenceRange(
  text: string,
  cursor: number,
): MentionRange | null {
  const end = Math.min(Math.max(cursor, 0), text.length);
  const prefix = text.slice(0, end);
  const start = prefix.lastIndexOf("#");
  if (start < 0) return null;
  const query = text.slice(start + 1, end);
  if (query.length > 240 || /[\n\r]/.test(query)) return null;
  return { start, end, query };
}

export function readSlashCommandRange(text: string, cursor: number): MentionRange | null {
  const end = Math.min(Math.max(cursor, 0), text.length);
  const prefix = text.slice(0, end);
  const lineStart = Math.max(prefix.lastIndexOf("\n") + 1, 0);
  const beforeLine = text.slice(lineStart, end);
  if (!beforeLine.startsWith("/")) return null;
  if (beforeLine.length > 160 || /\s/.test(beforeLine)) return null;
  return { start: lineStart, end, query: beforeLine.slice(1) };
}

