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

export interface DraftMarkdownListNode {
  ordered: boolean;
  start: number | null;
  items: DraftMarkdownListItem[];
}

export interface DraftMarkdownListItem {
  lines: string[];
  taskChecked: boolean | null;
  children: DraftMarkdownListNode[];
}

export interface FencedCodeBlock {
  text: string;
  language: string;
  nextIndex: number;
  closed: boolean;
}

export interface MathBlock {
  text: string;
  raw: string;
  nextIndex: number;
  closed: boolean;
}

export interface ParsedListItem {
  indent: number;
  ordered: boolean;
  number: number | null;
  text: string;
  taskChecked: boolean | null;
}

export function makeBlock(
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

export function makeInlineToken(
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
