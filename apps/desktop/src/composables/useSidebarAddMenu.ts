import { nextTick, onBeforeUnmount, ref, watch } from "vue";

const MENU_W = 200;
const MENU_H_EST = 132;

export function useSidebarAddMenu() {
  const addMenuOpen = ref(false);
  const menuPos = ref<{ x: number; y: number }>({ x: 0, y: 0 });

  function openAddMenu(e: MouseEvent) {
    const x = Math.min(e.clientX, window.innerWidth - MENU_W - 4);
    const y = Math.min(e.clientY, window.innerHeight - MENU_H_EST - 4);
    menuPos.value = { x: Math.max(4, x), y: Math.max(4, y) };
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

  watch(addMenuOpen, async (v) => {
    if (v) {
      await nextTick();
      document.addEventListener("pointerdown", onDocPointer, true);
      document.addEventListener("keydown", onDocKey);
    } else {
      document.removeEventListener("pointerdown", onDocPointer, true);
      document.removeEventListener("keydown", onDocKey);
    }
  });

  onBeforeUnmount(() => {
    document.removeEventListener("pointerdown", onDocPointer, true);
    document.removeEventListener("keydown", onDocKey);
  });

  return {
    addMenuOpen,
    closeAddMenu,
    menuPos,
    openAddMenu,
  };
}
