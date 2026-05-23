import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/vue";
import { afterEach, beforeEach, vi } from "vitest";
import { resetTauriMockData, mockInvoke } from "./tauriMock";

vi.mock("@tauri-apps/api/event", async () => {
  const { mockListen } = await import("./tauriMock");
  return { listen: mockListen };
});

Object.defineProperty(window, "__TAURI_INTERNALS__", {
  configurable: true,
  value: {
    invoke: mockInvoke,
    transformCallback: vi.fn(() => 1),
  },
});

beforeEach(() => {
  resetTauriMockData();
});

afterEach(() => {
  cleanup();
});
