import type { Directive } from "vue";
import type {
  ContextMenuItem,
  ContextMenuProvider,
} from "../composables/useContextMenu";
import { measurePerfAsync } from "../utils/perf";
import { createLazyLoadState } from "../utils/lazyLoadState";

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
const bindSeq = new WeakMap<Element, number>();
const registerContextMenuLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "context-menu.directive.load",
    async () => (await import("../composables/useContextMenu")).registerContextMenu,
    { detail: "registerContextMenu" },
  )
);

function nextBindSeq(el: Element): number {
  const seq = (bindSeq.get(el) ?? 0) + 1;
  bindSeq.set(el, seq);
  return seq;
}

function currentBindSeq(el: Element): number {
  return bindSeq.get(el) ?? 0;
}

function clearBinding(el: Element) {
  cleanups.get(el)?.();
  cleanups.delete(el);
  nextBindSeq(el);
}

function loadRegisterContextMenu() {
  return registerContextMenuLoad.load();
}

function rebind(el: Element, value: Value) {
  clearBinding(el);
  if (!value) return;
  const seq = currentBindSeq(el);
  const provider: ContextMenuProvider =
    typeof value === "function" ? value : () => value;
  void loadRegisterContextMenu()
    .then((registerContextMenu) => {
      if (currentBindSeq(el) !== seq || cleanups.has(el)) return;
      cleanups.set(el, registerContextMenu(el, provider));
    })
    .catch((err) => {
      console.error("[context-menu] load registerContextMenu failed", err);
    });
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
    clearBinding(el);
  },
};
