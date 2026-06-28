import { describe, expect, it, vi } from "vitest";
import {
  addDomEventListener,
  installCombinedUnlisten,
  installUnlistenFns,
} from "../src/utils/eventListeners";

describe("event listener lifecycle helpers", () => {
  it("rolls back already installed listeners when a later installer fails", async () => {
    const calls: string[] = [];

    await expect(installUnlistenFns([
      async () => {
        calls.push("install-a");
        return () => calls.push("unlisten-a");
      },
      async () => {
        calls.push("install-b");
        return () => calls.push("unlisten-b");
      },
      async () => {
        calls.push("install-c");
        throw new Error("install-c failed");
      },
    ])).rejects.toThrow("install-c failed");

    expect(calls).toEqual([
      "install-a",
      "install-b",
      "install-c",
      "unlisten-b",
      "unlisten-a",
    ]);
  });

  it("returns an idempotent combined unlisten function", async () => {
    const calls: string[] = [];
    const unlisten = await installCombinedUnlisten([
      async () => () => calls.push("unlisten-a"),
      async () => () => calls.push("unlisten-b"),
    ]);

    unlisten();
    unlisten();

    expect(calls).toEqual(["unlisten-b", "unlisten-a"]);
  });

  it("continues cleanup when one unlisten throws", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const calls: string[] = [];
    const unlisten = await installCombinedUnlisten([
      async () => () => calls.push("unlisten-a"),
      async () => () => {
        calls.push("unlisten-b");
        throw new Error("unlisten-b failed");
      },
      async () => () => calls.push("unlisten-c"),
    ]);

    unlisten();

    expect(calls).toEqual(["unlisten-c", "unlisten-b", "unlisten-a"]);
    expect(errorSpy).toHaveBeenCalledOnce();
    errorSpy.mockRestore();
  });

  it("returns cleanup for DOM listeners", () => {
    const target = new EventTarget();
    const listener = vi.fn();
    const removeSpy = vi.spyOn(target, "removeEventListener");

    const unlisten = addDomEventListener(target, "test-event", listener);
    target.dispatchEvent(new Event("test-event"));
    unlisten();
    target.dispatchEvent(new Event("test-event"));

    expect(listener).toHaveBeenCalledTimes(1);
    expect(removeSpy).toHaveBeenCalledWith("test-event", listener, undefined);
  });
});

