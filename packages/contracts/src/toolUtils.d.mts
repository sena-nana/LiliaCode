export function isRecord(value: unknown): value is Record<string, unknown>;
export function readRecord(value: unknown): Record<string, unknown>;
export function pick(record: Record<string, unknown>, keys: string[]): unknown;
export function stringOrNull(value: unknown): string | null;
export function shortText(value: unknown, max: number): string;
export function stringifyInline(value: unknown): string;
export function compactLine(value: unknown, max?: number): string;
export function readFirstString(
  payload: unknown,
  keys: string[],
  max?: number,
): string;
export function readFirstText(
  payload: unknown,
  keys: string[],
  max?: number,
): string;
export function pickString(
  record: unknown,
  keys: string[],
  max?: number,
): string;
export function pickNumber(record: unknown, keys: string[]): number | undefined;
export function parseRecordJson(value: unknown): Record<string, unknown> | null;
export function readArrayRecords(value: unknown): Record<string, unknown>[];
