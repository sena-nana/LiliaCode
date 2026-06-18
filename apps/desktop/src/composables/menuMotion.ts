export interface AnchoredMenuPosition {
  x: number;
  y: number;
  anchorX: number;
  anchorY: number;
}

export interface MenuTransformOrigin {
  x: number;
  y: number;
}

export type MenuPlacement = "top" | "bottom";
export type MenuAlignment = "start" | "end" | "center";
export type AnchoredMenuPlacement =
  | "top-start"
  | "top-end"
  | "top-center"
  | "bottom-start"
  | "bottom-end"
  | "bottom-center";

export interface AnchoredRect {
  left: number;
  top: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
}

export interface AnchoredOverlayLayout extends AnchoredMenuPosition {
  placement: MenuPlacement;
  alignment: MenuAlignment;
}

export const SB_MENU_EDGE_PADDING = 4;
export const SB_MENU_POP_TRANSITION_MS = 180;

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function horizontalPosition(
  rect: AnchoredRect,
  width: number,
  alignment: MenuAlignment,
): number {
  if (alignment === "end") return rect.right - width;
  if (alignment === "center") return rect.left + (rect.width - width) / 2;
  return rect.left;
}

export function createAnchoredMenuPosition(
  x: number,
  y: number,
  anchorX = x,
  anchorY = y,
): AnchoredMenuPosition {
  return { x, y, anchorX, anchorY };
}

export function clampAnchoredMenuPosition(
  position: AnchoredMenuPosition,
  width: number,
  height: number,
): AnchoredMenuPosition {
  const maxX = Math.max(SB_MENU_EDGE_PADDING, window.innerWidth - width - SB_MENU_EDGE_PADDING);
  const maxY = Math.max(SB_MENU_EDGE_PADDING, window.innerHeight - height - SB_MENU_EDGE_PADDING);
  return {
    ...position,
    x: clamp(position.x, SB_MENU_EDGE_PADDING, maxX),
    y: clamp(position.y, SB_MENU_EDGE_PADDING, maxY),
  };
}

export function resolveMenuTransformOrigin(
  position: AnchoredMenuPosition,
  width = Number.POSITIVE_INFINITY,
  height = Number.POSITIVE_INFINITY,
): MenuTransformOrigin {
  return {
    x: clamp(position.anchorX - position.x, 0, width),
    y: clamp(position.anchorY - position.y, 0, height),
  };
}

export function parseAnchoredMenuPlacement(
  placement: AnchoredMenuPlacement,
): { placement: MenuPlacement; alignment: MenuAlignment } {
  const [vertical, horizontal] = placement.split("-") as [MenuPlacement, MenuAlignment];
  return { placement: vertical, alignment: horizontal };
}

export function createAnchoredRect(
  left: number,
  top: number,
  width = 0,
  height = 0,
): AnchoredRect {
  return {
    left,
    top,
    right: left + width,
    bottom: top + height,
    width,
    height,
  };
}

export function rectFromDomRect(rect: DOMRect): AnchoredRect {
  return createAnchoredRect(rect.left, rect.top, rect.width, rect.height);
}

export function resolveAnchoredOverlayLayout(options: {
  anchorRect: AnchoredRect;
  overlayWidth: number;
  overlayHeight: number;
  preferredPlacement: AnchoredMenuPlacement;
  anchorPoint?: { x: number; y: number } | null;
  offset?: number;
}): AnchoredOverlayLayout {
  const { anchorRect, overlayWidth, overlayHeight, preferredPlacement } = options;
  const offset = options.offset ?? 6;
  const parsed = parseAnchoredMenuPlacement(preferredPlacement);
  const above = anchorRect.top - SB_MENU_EDGE_PADDING - offset;
  const below = window.innerHeight - anchorRect.bottom - SB_MENU_EDGE_PADDING - offset;
  const fitsTop = overlayHeight <= above;
  const fitsBottom = overlayHeight <= below;
  const placement = parsed.placement === "top"
    ? (fitsTop || (!fitsBottom && above >= below) ? "top" : "bottom")
    : (fitsBottom || (!fitsTop && below >= above) ? "bottom" : "top");
  const unclamped = createAnchoredMenuPosition(
    horizontalPosition(anchorRect, overlayWidth, parsed.alignment),
    placement === "top"
      ? anchorRect.top - overlayHeight - offset
      : anchorRect.bottom + offset,
    options.anchorPoint?.x ?? (
      parsed.alignment === "end"
        ? anchorRect.right
        : parsed.alignment === "center"
          ? anchorRect.left + anchorRect.width / 2
          : anchorRect.left
    ),
    options.anchorPoint?.y ?? (placement === "top" ? anchorRect.top : anchorRect.bottom),
  );
  const clamped = clampAnchoredMenuPosition(unclamped, overlayWidth, overlayHeight);
  return {
    ...clamped,
    placement,
    alignment: parsed.alignment,
  };
}
