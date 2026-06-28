import { afterEach, describe, expect, it, vi } from "vitest";

describe("main bootstrap installers", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("可取消尚未进入 paint 的全局安装器调度", async () => {
    const cancelAnimationFrame = vi.fn();
    vi.stubGlobal("requestAnimationFrame", vi.fn(() => 37));
    vi.stubGlobal("cancelAnimationFrame", cancelAnimationFrame);
    const { scheduleGlobalInstallers } = await import("../src/mainBootstrap");

    const cancel = scheduleGlobalInstallers();
    cancel();

    expect(cancelAnimationFrame).toHaveBeenCalledWith(37);
  });
});

