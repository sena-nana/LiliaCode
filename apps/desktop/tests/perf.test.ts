import { afterEach, describe, expect, it, vi } from "vitest";
import {
  scheduleAfterPaint,
} from "../src/utils/perf";

describe("perf utilities", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("cancels a callback queued after animation frame", () => {
    vi.useFakeTimers();
    let frameCallback: FrameRequestCallback | null = null;
    vi.stubGlobal("requestAnimationFrame", vi.fn((callback: FrameRequestCallback) => {
      frameCallback = callback;
      return 1;
    }));
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
    const callback = vi.fn();

    const cancel = scheduleAfterPaint(callback);
    frameCallback?.(0);
    cancel();
    vi.runAllTimers();

    expect(callback).not.toHaveBeenCalled();
  });
});

