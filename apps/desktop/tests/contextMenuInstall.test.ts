import { afterEach, describe, expect, it, vi } from "vitest";
import {
  installContextMenu,
  uninstallContextMenu,
} from "../src/composables/useContextMenuInstall";

describe("context menu global installer", () => {
  afterEach(() => {
    uninstallContextMenu();
    vi.restoreAllMocks();
  });

  it("returns cleanup for all global listeners", () => {
    const addSpy = vi.spyOn(window, "addEventListener");
    const removeSpy = vi.spyOn(window, "removeEventListener");

    const cleanup = installContextMenu();
    expect(cleanup).toBeTypeOf("function");
    expect(addSpy).toHaveBeenCalledTimes(6);

    cleanup?.();
    cleanup?.();

    expect(removeSpy).toHaveBeenCalledTimes(6);
  });
});

