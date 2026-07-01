export interface DropPoint {
  x: number;
  y: number;
}

export interface DropPayload {
  type: string;
  paths: string[];
  position: DropPoint | null;
}

export function isPointInsideElement(
  point: DropPoint | null,
  element: HTMLElement | null,
): boolean {
  if (!point || !element) return false;
  const rect = element.getBoundingClientRect();
  return point.x >= rect.left &&
    point.x <= rect.right &&
    point.y >= rect.top &&
    point.y <= rect.bottom;
}

export function readDropPayload(payload: unknown): DropPayload | null {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const row = payload as Record<string, unknown>;
  const paths = Array.isArray(row.paths)
    ? row.paths.filter((path): path is string => typeof path === "string")
    : [];
  const position = row.position && typeof row.position === "object" && !Array.isArray(row.position)
    ? row.position as Record<string, unknown>
    : null;
  const x = typeof position?.x === "number" ? position.x : null;
  const y = typeof position?.y === "number" ? position.y : null;
  return {
    type: typeof row.type === "string" ? row.type : "",
    paths,
    position: x === null || y === null ? null : { x, y },
  };
}

export async function normalizeDropPoint(
  point: DropPoint | null,
  scaleFactor: () => Promise<number>,
): Promise<DropPoint | null> {
  if (!point) return null;
  try {
    const factor = await scaleFactor();
    if (!Number.isFinite(factor) || factor <= 0) return point;
    return {
      x: point.x / factor,
      y: point.y / factor,
    };
  } catch {
    return point;
  }
}
