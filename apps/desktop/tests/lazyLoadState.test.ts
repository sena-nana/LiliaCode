import { describe, expect, it, vi } from "vitest";
import { createLazyLoadState } from "../src/utils/lazyLoadState";

describe("lazy load state", () => {
  it("caches successful loads", async () => {
    const loader = vi.fn(async () => ({ value: 1 }));
    const state = createLazyLoadState(loader);

    const first = await state.load();
    const second = await state.load();

    expect(first).toBe(second);
    expect(loader).toHaveBeenCalledTimes(1);
    expect(state.status.value).toBe("loaded");
    expect(state.loaded.value).toBe(true);
  });

  it("keeps failure state and retries after a failed load", async () => {
    const loader = vi.fn()
      .mockRejectedValueOnce(new Error("chunk failed"))
      .mockResolvedValueOnce("ok");
    const state = createLazyLoadState(loader);

    await expect(state.load()).rejects.toThrow("chunk failed");
    expect(state.status.value).toBe("error");
    expect(state.failed.value).toBe(true);
    expect(state.error.value).toBeInstanceOf(Error);

    await expect(state.retry()).resolves.toBe("ok");
    expect(loader).toHaveBeenCalledTimes(2);
    expect(state.status.value).toBe("loaded");
    expect(state.error.value).toBeNull();
  });

  it("shares concurrent pending loads", async () => {
    let resolveLoad: (value: string) => void = () => undefined;
    const loader = vi.fn(() => new Promise<string>((resolve) => {
      resolveLoad = resolve;
    }));
    const state = createLazyLoadState(loader);

    const first = state.load();
    const second = state.load();
    resolveLoad("ready");

    await expect(Promise.all([first, second])).resolves.toEqual(["ready", "ready"]);
    expect(loader).toHaveBeenCalledTimes(1);
  });
});

