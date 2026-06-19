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

vi.mock("@tauri-apps/api/path", () => ({
  homeDir: vi.fn(async () => "C:\\Users\\mock"),
}));

Object.defineProperty(window, "__TAURI_INTERNALS__", {
  configurable: true,
  value: {
    invoke: mockInvoke,
    transformCallback: vi.fn(() => 1),
    convertFileSrc: vi.fn((path: string) => `asset://${path.replace(/\\/g, "/")}`),
  },
});

Object.defineProperty(HTMLCanvasElement.prototype, "getContext", {
  configurable: true,
  value(this: HTMLCanvasElement) {
    const context: Record<string, unknown> = {
      canvas: this,
      createLinearGradient: vi.fn(() => ({ addColorStop: vi.fn() })),
      createPattern: vi.fn(() => null),
      createRadialGradient: vi.fn(() => ({ addColorStop: vi.fn() })),
      getImageData: vi.fn(() => ({ data: new Uint8ClampedArray(4) })),
      getLineDash: vi.fn(() => []),
      measureText: vi.fn((text: string) => ({
        width: text.length * 6,
        actualBoundingBoxAscent: 8,
        actualBoundingBoxDescent: 2,
      })),
    };
    return new Proxy(context, {
      get(target, prop: string) {
        target[prop] ??= vi.fn();
        return target[prop];
      },
    });
  },
});

class MockResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

Object.defineProperty(window, "ResizeObserver", {
  configurable: true,
  value: MockResizeObserver,
});

beforeEach(async () => {
  resetTauriMockData();
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: {
      writeText: vi.fn(async () => undefined),
    },
  });
  const [{ PROJECTS }, tasksModule, milestonesModule] = await Promise.all([
    import("../src/data/projects"),
    import("../src/data/tasks"),
    import("../src/data/milestones"),
  ]);
  const { useSidebarDisplayMode } = await import("../src/composables/useSidebarDisplayMode");
  useSidebarDisplayMode().setSidebarDisplayMode("grouped");
  const { useCornerStyle } = await import("../src/composables/useCornerStyle");
  const { setCornerRadius, setCornerStyle } = useCornerStyle();
  setCornerStyle("smooth");
  setCornerRadius(8);
  const { clearGitHubRepoCache } = await import("../src/services/projects");
  clearGitHubRepoCache();
  const {
    ORPHAN_LIST,
    ORPHANS_LOADED,
    PROJECT_TASKS_LOADED,
    TASKS,
    installTasksChangedListener,
  } = tasksModule;
  installTasksChangedListener();
  PROJECTS.value = mockProjectsForStore();
  TASKS.value = mockTasksByProjectForStore();
  ORPHAN_LIST.value = mockOrphansForStore();
  PROJECT_TASKS_LOADED.value = {};
  ORPHANS_LOADED.value = false;
  milestonesModule.MILESTONES.value = {};
  milestonesModule.MILESTONE_LINKS.value = {};
  milestonesModule.PROJECT_ROADMAP_LOADED.value = {};
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
