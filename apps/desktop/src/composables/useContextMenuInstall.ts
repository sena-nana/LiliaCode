import {
  closeContextMenu,
  isContextMenuOpen,
  syncContextMenuForEvent,
} from "./useContextMenu";

let installed = false;

export function installContextMenu() {
  if (installed || typeof window === "undefined") return;
  installed = true;

  window.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    syncContextMenuForEvent(e);
  });

  window.addEventListener(
    "pointerdown",
    (e) => {
      if (!isContextMenuOpen()) return;
      const target = e.target as Element | null;
      if (target?.closest?.(".ctx-menu")) return;
      closeContextMenu();
    },
    true,
  );

  window.addEventListener("keydown", (e) => {
    if (e.key !== "Escape" || !isContextMenuOpen()) return;
    closeContextMenu();
    e.stopPropagation();
  });

  window.addEventListener(
    "scroll",
    () => {
      if (isContextMenuOpen()) closeContextMenu();
    },
    true,
  );
  window.addEventListener("resize", () => {
    if (isContextMenuOpen()) closeContextMenu();
  });
  window.addEventListener("blur", closeContextMenu);
}
