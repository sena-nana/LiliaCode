import {
  closeContextMenu,
  isContextMenuOpen,
  syncContextMenuForEvent,
} from "./useContextMenu";
import { addDomEventListener, runUnlistenFns } from "../utils/eventListeners";

let installed = false;
let unlisteners: Array<() => void> = [];

export function uninstallContextMenu() {
  if (!installed || typeof window === "undefined") return;
  installed = false;
  runUnlistenFns(unlisteners.splice(0).reverse());
}

export function installContextMenu() {
  if (installed || typeof window === "undefined") return uninstallContextMenu;
  installed = true;

  unlisteners = [
    addDomEventListener(window, "contextmenu", (e) => {
      e.preventDefault();
      syncContextMenuForEvent(e);
    }),

    addDomEventListener(window, "pointerdown", (e) => {
      if (!isContextMenuOpen()) return;
      const target = e.target as Element | null;
      if (target?.closest?.(".ctx-menu")) return;
      closeContextMenu();
    }, true),

    addDomEventListener(window, "keydown", (e) => {
      if (e.key !== "Escape" || !isContextMenuOpen()) return;
      closeContextMenu();
      e.stopPropagation();
    }),

    addDomEventListener(window, "scroll", () => {
      if (isContextMenuOpen()) closeContextMenu();
    }, true),
    addDomEventListener(window, "resize", () => {
      if (isContextMenuOpen()) closeContextMenu();
    }),
    addDomEventListener(window, "blur", closeContextMenu),
  ];
  return uninstallContextMenu;
}
