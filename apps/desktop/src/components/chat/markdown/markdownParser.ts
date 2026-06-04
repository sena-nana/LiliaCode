export type {
  InlineToken,
  InlineTokenType,
  MarkdownBlockNode,
  MarkdownBlockType,
  MarkdownListItem,
  MarkdownListNode,
  TableAlignment,
} from "./types";
export { parseMarkdownBlocks } from "./block";
export { parseInlineMarkdown } from "./inline";

export function normalizeMarkdownSource(content: string | null | undefined): string {
  return (content ?? "").replace(/\r\n?/g, "\n").trim();
}

export function toSingleLineText(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}
