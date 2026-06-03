import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/vue";
import { afterEach, beforeEach, vi } from "vitest";
import {
  bindMockProjectPinUpdater,
  mockInvoke,
  mockOrphansForStore,
  mockProjectsForStore,
  resetTauriMockData,
  mockTasksByProjectForStore,
} from "./tauriMock";

vi.mock("@tauri-apps/api/event", async () => {
  const { mockListen } = await import("./tauriMock");
  return { listen: mockListen };
});

vi.mock("@tauri-apps/api/webview", async () => {
  const { mockGetCurrentWebview } = await import("./tauriMock");
  return { getCurrentWebview: mockGetCurrentWebview };
});

vi.mock("@tauri-apps/api/window", async () => {
  const { mockGetCurrentWindow } = await import("./tauriMock");
  return { getCurrentWindow: mockGetCurrentWindow };
});

Object.defineProperty(window, "__TAURI_INTERNALS__", {
  configurable: true,
  value: {
    invoke: mockInvoke,
    transformCallback: vi.fn(() => 1),
    convertFileSrc: vi.fn((path: string) => `asset://${path.replace(/\\/g, "/")}`),
  },
});

beforeEach(async () => {
  resetTauriMockData();
  const [{ PROJECTS }, tasksModule] = await Promise.all([
    import("../src/data/projects"),
    import("../src/data/tasks"),
  ]);
  const { ORPHAN_LIST, TASKS, installTasksChangedListener } = tasksModule;
  installTasksChangedListener();
  PROJECTS.value = mockProjectsForStore();
  TASKS.value = mockTasksByProjectForStore();
  ORPHAN_LIST.value = mockOrphansForStore();
  bindMockProjectPinUpdater((projectId, pinned) => {
    PROJECTS.value = PROJECTS.value.map((project) =>
      project.id === projectId ? { ...project, pinned } : project
    );
  });
});

afterEach(() => {
  bindMockProjectPinUpdater(null);
  cleanup();
});
