import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import {
  createAnchoredMenuPosition,
  type AnchoredMenuPosition,
} from "./menuMotion";
import { addDomEventListener, runUnlistenFns } from "../utils/eventListeners";

export function useSidebarAddMenu() {
  const addMenuOpen = ref(false);
  const menuPos = ref<AnchoredMenuPosition>(createAnchoredMenuPosition(0, 0));
  let listenerSeq = 0;
  let documentUnlisteners: Array<() => void> = [];

  function openAddMenu(e: MouseEvent) {
    menuPos.value = createAnchoredMenuPosition(
      e.clientX,
      e.clientY,
      e.clientX,
      e.clientY,
    );
    addMenuOpen.value = true;
  }

  function closeAddMenu() {
    addMenuOpen.value = false;
  }

  function onDocPointer(e: PointerEvent) {
    const target = e.target as HTMLElement | null;
    if (target && target.closest && target.closest(".sb-menu")) return;
    closeAddMenu();
  }

  function onDocKey(e: KeyboardEvent) {
    if (e.key === "Escape" && addMenuOpen.value) {
      closeAddMenu();
      e.stopPropagation();
    }
  }

  function clearDocumentListeners() {
    runUnlistenFns(documentUnlisteners.splice(0).reverse());
  }

  function installDocumentListeners() {
    clearDocumentListeners();
    documentUnlisteners = [
      addDomEventListener(document, "pointerdown", onDocPointer, true),
      addDomEventListener(document, "keydown", onDocKey),
    ];
  }

  watch(addMenuOpen, async (v) => {
    const seq = ++listenerSeq;
    clearDocumentListeners();
    if (v) {
      await nextTick();
      if (seq !== listenerSeq || !addMenuOpen.value) return;
      installDocumentListeners();
    }
  });

  onBeforeUnmount(() => {
    listenerSeq += 1;
    clearDocumentListeners();
  });

  return {
    addMenuOpen,
    closeAddMenu,
    menuPos,
    openAddMenu,
  };
}

