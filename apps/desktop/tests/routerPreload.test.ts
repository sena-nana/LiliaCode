import { afterEach, describe, expect, it, vi } from "vitest";

describe("router task detail preload", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  it("取消尚未进入 paint 的任务详情预加载", async () => {
    const cancelAnimationFrame = vi.fn();
    vi.stubGlobal("requestAnimationFrame", vi.fn(() => 19));
    vi.stubGlobal("cancelAnimationFrame", cancelAnimationFrame);
    const {
      cancelTaskDetailPreloadSchedule,
      scheduleTaskDetailPreload,
    } = await import("../src/router");

    scheduleTaskDetailPreload("test");
    cancelTaskDetailPreloadSchedule();

    expect(cancelAnimationFrame).toHaveBeenCalledWith(19);
  });

  it("取消已经进入 idle 等待的任务详情预加载", async () => {
    vi.useFakeTimers();
    const cancelIdleCallback = vi.fn();
    vi.stubGlobal("requestAnimationFrame", vi.fn((callback: FrameRequestCallback) => {
      callback(0);
      return 23;
    }));
    vi.stubGlobal("requestIdleCallback", vi.fn(() => 29));
    vi.stubGlobal("cancelIdleCallback", cancelIdleCallback);
    const {
      cancelTaskDetailPreloadSchedule,
      scheduleTaskDetailPreload,
    } = await import("../src/router");

    scheduleTaskDetailPreload("test");
    await vi.runOnlyPendingTimersAsync();
    cancelTaskDetailPreloadSchedule();

    expect(cancelIdleCallback).toHaveBeenCalledWith(29);
  });
});
