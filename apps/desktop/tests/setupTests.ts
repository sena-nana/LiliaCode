import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/vue";
import { afterEach, beforeEach } from "vitest";
import { resetTauriMockData, mockInvoke } from "./tauriMock";

Object.defineProperty(window, "__TAURI_INTERNALS__", {
  configurable: true,
  value: {
    invoke: mockInvoke,
  },
});

beforeEach(() => {
  resetTauriMockData();
});

afterEach(() => {
  cleanup();
});
