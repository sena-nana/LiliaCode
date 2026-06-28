import { waitFor } from "@testing-library/vue";
import { describe, expect, it, vi } from "vitest";
import { TASKS_CHANGED_EVENT_NAME } from "@lilia/contracts";
import {
  failNextMockListen,
  mockListenerCount,
} from "./tauriMock";

describe("global data listener installation", () => {
  it("keeps tasks changed listener installation idempotent and retryable", async () => {
    vi.resetModules();
    const baseline = mockListenerCount(TASKS_CHANGED_EVENT_NAME);
    failNextMockListen(TASKS_CHANGED_EVENT_NAME, "tasks listener failed");
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const module = await import("../src/data/tasks");

    await waitFor(() => {
      expect(errorSpy).toHaveBeenCalledWith(
        `[tasks] listen ${TASKS_CHANGED_EVENT_NAME} failed`,
        expect.any(Error),
      );
    });
    expect(mockListenerCount(TASKS_CHANGED_EVENT_NAME)).toBe(baseline);

    module.installTasksChangedListener();
    await waitFor(() => {
      expect(mockListenerCount(TASKS_CHANGED_EVENT_NAME)).toBe(baseline + 1);
    });

    module.installTasksChangedListener();
    expect(mockListenerCount(TASKS_CHANGED_EVENT_NAME)).toBe(baseline + 1);
    errorSpy.mockRestore();
  });

  it("retries sidebar conversation listener registration after an initial failure", async () => {
    vi.resetModules();
    const baseline = mockListenerCount(TASKS_CHANGED_EVENT_NAME);
    failNextMockListen(TASKS_CHANGED_EVENT_NAME, "sidebar listener failed");
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const module = await import("../src/data/sidebarConversations");

    await waitFor(() => {
      expect(errorSpy).toHaveBeenCalledWith(
        `[sidebar-conversations] listen ${TASKS_CHANGED_EVENT_NAME} failed`,
        expect.any(Error),
      );
    });
    expect(mockListenerCount(TASKS_CHANGED_EVENT_NAME)).toBe(baseline);

    await module.ensureSidebarConversationsLoaded(true);

    await waitFor(() => {
      expect(mockListenerCount(TASKS_CHANGED_EVENT_NAME)).toBe(baseline + 1);
    });
    errorSpy.mockRestore();
  });

  it("retries project dashboard listener registration after an initial failure", async () => {
    vi.resetModules();
    const baseline = mockListenerCount(TASKS_CHANGED_EVENT_NAME);
    failNextMockListen(TASKS_CHANGED_EVENT_NAME, "dashboard listener failed");
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const module = await import("../src/data/projectDashboard");

    await waitFor(() => {
      expect(errorSpy).toHaveBeenCalledWith(
        `[project-dashboard] listen ${TASKS_CHANGED_EVENT_NAME} failed`,
        expect.any(Error),
      );
    });
    expect(mockListenerCount(TASKS_CHANGED_EVENT_NAME)).toBe(baseline);

    await module.ensureProjectDashboardLoaded(true);

    await waitFor(() => {
      expect(mockListenerCount(TASKS_CHANGED_EVENT_NAME)).toBe(baseline + 1);
    });
    errorSpy.mockRestore();
  });
});

