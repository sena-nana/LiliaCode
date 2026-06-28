import { reactive, type Component } from "vue";
import { createAnchoredMenuPosition } from "./menuMotion";

export interface ContextMenuItem {
  /** 可选稳定 key，没给就用 index。 */
  id?: string;
  label: string;
  /** 可选图标组件（@lucide/vue 之类）。 */
  icon?: Component;
  disabled?: boolean;
  /** 红色危险项（删除之类）。 */
  danger?: boolean;
  confirmLabel?: string;
  onSelect: () => void;
}

/**
 * 注册到 DOM 元素上的右键提供者：返回该元素愿意提供的菜单项。
 * - 返回 null / undefined / 空数组：表示「我不接管」，框架继续向外问祖先。
 * - 第一个返回非空数组的元素胜出。
 */
export type ContextMenuProvider = (e: MouseEvent) => ContextMenuItem[] | null | undefined;

interface MenuState {
  open: boolean;
  x: number;
  y: number;
  anchorX: number;
  anchorY: number;
  items: ContextMenuItem[];
  /** 当前正等待二次确认的菜单项 key（item.id ?? ""）。 */
  pendingConfirmId: string | null;
  openSeq: number;
}

const state = reactive<MenuState>({
  open: false,
  x: 0,
  y: 0,
  anchorX: 0,
  anchorY: 0,
  items: [],
  pendingConfirmId: null,
  openSeq: 0,
});

const providers = new WeakMap<Element, ContextMenuProvider>();

export function registerContextMenu(el: Element, provider: ContextMenuProvider) {
  providers.set(el, provider);
  return () => providers.delete(el);
}

function openMenu(x: number, y: number, items: ContextMenuItem[]) {
  const position = createAnchoredMenuPosition(x, y);
  state.items = items;
  state.x = position.x;
  state.y = position.y;
  state.anchorX = position.anchorX;
  state.anchorY = position.anchorY;
  state.open = true;
  state.pendingConfirmId = null;
  state.openSeq += 1;
}

/**
 * 不走 right-click 的程序化入口：用在「更多」按钮 / kebab 图标这类
 * 用左键点开同一份菜单的场景。锚点 (x,y) 一般取按钮的 bottom-left
 * 或鼠标点击的 clientX/clientY，ContextMenuHost 会自己 clamp 视口。
 */
export function openContextMenuAt(
  x: number,
  y: number,
  items: ContextMenuItem[],
) {
  if (!items.length) return;
  openMenu(x, y, items);
}

export function closeContextMenu() {
  if (!state.open) return;
  state.open = false;
}

export function finalizeClosedContextMenu() {
  if (state.open) return;
  state.items = [];
  state.pendingConfirmId = null;
}

export function isContextMenuItemPending(item: ContextMenuItem): boolean {
  if (!item.confirmLabel) return false;
  return state.pendingConfirmId === (item.id ?? "");
}

export function selectContextMenuItem(item: ContextMenuItem) {
  if (item.disabled) return;
  const key = item.id ?? "";
  if (item.confirmLabel && state.pendingConfirmId !== key) {
    // 首次点选 / 切到另一个确认项：进入 pending，不关菜单也不执行。
    state.pendingConfirmId = key;
    return;
  }
  closeContextMenu();
  item.onSelect();
}

function collectItemsFor(e: MouseEvent): ContextMenuItem[] {
  let node: Element | null = e.target as Element | null;
  while (node) {
    const p = providers.get(node);
    if (p) {
      const items = p(e);
      if (items && items.length) return items;
    }
    node = node.parentElement;
  }
  return [];
}

export function syncContextMenuForEvent(e: MouseEvent) {
  const items = collectItemsFor(e);
  if (items.length) openMenu(e.clientX, e.clientY, items);
  else closeContextMenu();
}

export function isContextMenuOpen() {
  return state.open;
}

export function useContextMenu() {
  return {
    /** 注意：state 是模块级 reactive，外部读不要直接写；要变动用导出的函数。 */
    state,
    close: closeContextMenu,
    finalizeClose: finalizeClosedContextMenu,
    select: selectContextMenuItem,
  };
}


