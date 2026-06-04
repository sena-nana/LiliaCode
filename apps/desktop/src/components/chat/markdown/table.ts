import type { TableAlignment } from "./types";

export interface ParsedTable {
  headers: string[];
  alignments: TableAlignment[];
  rows: string[][];
  nextIndex: number;
}

export function isTableStart(lines: string[], startIndex: number): boolean {
  return parseTable(lines, startIndex) !== null;
}

export function parseTable(
  lines: string[],
  startIndex: number,
): ParsedTable | null {
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
