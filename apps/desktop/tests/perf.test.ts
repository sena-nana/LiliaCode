import { afterEach, describe, expect, it, vi } from "vitest";
import {
  beginPerfStage,
  measurePerfAsync,
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

  it("logs async failures with feature route id and seq context", async () => {
    const err = new Error("boom");
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(measurePerfAsync(
      "chat.context.hydrate",
      async () => {
        throw err;
      },
      {
        feature: "chat",
        id: "task-1",
        route: "/projects/project-1/tasks/task-1",
        seq: 7,
      },
    )).rejects.toThrow("boom");

    expect(consoleError).toHaveBeenCalledWith(
      expect.stringContaining("[perf] chat.context.hydrate:failed"),
      err,
    );
    const [message] = consoleError.mock.calls[0] ?? [];
    expect(message).toContain('feature="chat"');
    expect(message).toContain('route="/projects/project-1/tasks/task-1"');
    expect(message).toContain('id="task-1"');
    expect(message).toContain('seq="7"');
  });

  it("records replaced render stages as lightweight perf metrics", () => {
    const consoleInfo = vi.spyOn(console, "info").mockImplementation(() => {});

    const stage = beginPerfStage("route.paint", {
      feature: "route.paint",
      id: "/projects/project-1/tasks/task-1",
      route: "/projects/project-1/tasks/task-1",
      seq: 3,
    });
    stage.end("replaced");

    expect(consoleInfo).toHaveBeenCalledWith(expect.stringContaining("[perf] route.paint:replaced"));
    const [message] = consoleInfo.mock.calls[0] ?? [];
    expect(message).toContain('feature="route.paint"');
    expect(message).toContain('route="/projects/project-1/tasks/task-1"');
    expect(message).toContain('id="/projects/project-1/tasks/task-1"');
    expect(message).toContain('seq="3"');
  });
});

