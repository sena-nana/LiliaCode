import { reactive, type Component } from "vue";

export interface ContextMenuItem {
  /** 可选稳定 key，没给就用 index。 */
  id?: string;
  label: string;
  /** 可选图标组件（lucide-vue-next 之类）。 */
  icon?: Component;
  disabled?: boolean;
  /** 红色危险项（删除之类）。 */
  danger?: boolean;
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
  items: ContextMenuItem[];
}

const state = reactive<MenuState>({
  open: false,
  x: 0,
  y: 0,
  items: [],
});

const providers = new WeakMap<Element, ContextMenuProvider>();

export function registerContextMenu(el: Element, provider: ContextMenuProvider) {
  providers.set(el, provider);
  return () => providers.delete(el);
}

function openMenu(x: number, y: number, items: ContextMenuItem[]) {
  state.items = items;
  state.x = x;
  state.y = y;
  state.open = true;
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
  state.items = [];
}

export function selectContextMenuItem(item: ContextMenuItem) {
  if (item.disabled) return;
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

let installed = false;

/** 在 app 启动时调一次：屏蔽原生右键菜单，接管为自定义菜单。 */
export function installContextMenu() {
  if (installed || typeof window === "undefined") return;
  installed = true;

  window.addEventListener("contextmenu", (e) => {
    // 一律阻止浏览器默认菜单；有没有自定义菜单可弹，再单独判断。
    e.preventDefault();
    const items = collectItemsFor(e);
    if (items.length) openMenu(e.clientX, e.clientY, items);
    else closeContextMenu();
  });

  window.addEventListener(
    "pointerdown",
    (e) => {
      if (!state.open) return;
      const t = e.target as Element | null;
      if (t?.closest?.(".ctx-menu")) return;
      closeContextMenu();
    },
    true,
  );

  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && state.open) {
      closeContextMenu();
      e.stopPropagation();
    }
  });

  // 滚动 / resize 会让锚点失效，索性关掉。
  window.addEventListener(
    "scroll",
    () => {
      if (state.open) closeContextMenu();
    },
    true,
  );
  window.addEventListener("resize", () => {
    if (state.open) closeContextMenu();
  });

  // 窗口失焦时关掉，避免切走又切回还挂着。
  window.addEventListener("blur", () => closeContextMenu());
}

export function useContextMenu() {
  return {
    /** 注意：state 是模块级 reactive，外部读不要直接写；要变动用导出的函数。 */
    state,
    close: closeContextMenu,
    select: selectContextMenuItem,
  };
}
