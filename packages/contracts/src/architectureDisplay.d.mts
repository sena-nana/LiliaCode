export interface ArchitectureChangeDisplayOptions {
  fallback?: string;
  includeEdgeEndpoints?: boolean;
  removeEdgeLabel?: string;
  removeNodeLabel?: string;
  separator?: string;
  summaryLabel?: string;
  unnamedEdge?: string;
  unnamedNode?: string;
  upsertEdgeLabel?: string;
  upsertNodeLabel?: string;
}

export function readArchitectureChangeRecords(value: unknown): Record<string, unknown>[];

export function architectureChangeDisplayText(
  value: unknown,
  options?: ArchitectureChangeDisplayOptions,
): string;

export function architectureChangeCompactLabel(value: unknown): string;
