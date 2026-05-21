import type { Directive } from "vue";
import {
  registerContextMenu,
  type ContextMenuItem,
  type ContextMenuProvider,
} from "../composables/useContextMenu";

/**
 * v-context-menu 用法：
 *   <div v-context-menu="[{ label: '重命名', onSelect: ... }]" />
 *   <div v-context-menu="(e) => buildItems(e)" />
 *
 * 传函数会在右键时调用、可读到事件做条件分支；传数组则静态注册。
 * 返回空数组等于「不接管」，框架会继续向外问祖先。
 */
type Value = ContextMenuItem[] | ContextMenuProvider | null | undefined;

const cleanups = new WeakMap<Element, () => void>();

function rebind(el: Element, value: Value) {
  cleanups.get(el)?.();
  cleanups.delete(el);
  if (!value) return;
  const provider: ContextMenuProvider =
    typeof value === "function" ? value : () => value;
  cleanups.set(el, registerContextMenu(el, provider));
}

export const vContextMenu: Directive<Element, Value> = {
  mounted(el, binding) {
    rebind(el, binding.value);
  },
  updated(el, binding) {
    if (binding.value === binding.oldValue) return;
    rebind(el, binding.value);
  },
  beforeUnmount(el) {
    cleanups.get(el)?.();
    cleanups.delete(el);
  },
};
