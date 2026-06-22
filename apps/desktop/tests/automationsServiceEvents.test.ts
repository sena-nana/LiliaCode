import { describe, expect, it } from "vitest";
import {
  AUTOMATION_RUN_STARTED_EVENT_NAME,
  AUTOMATION_RUN_UPDATED_EVENT_NAME,
} from "@lilia/contracts";
import { onAutomationRunUpdated } from "../src/services/automations";
import {
  failNextMockListen,
  mockListenerCount,
} from "./tauriMock";

describe("automation service events", () => {
  it("cleans up already registered run listeners if a later registration fails", async () => {
    failNextMockListen(AUTOMATION_RUN_UPDATED_EVENT_NAME, "run updated listener failed");

    await expect(onAutomationRunUpdated(() => undefined)).rejects.toThrow(
      "run updated listener failed",
    );

    expect(mockListenerCount(AUTOMATION_RUN_STARTED_EVENT_NAME)).toBe(0);
    expect(mockListenerCount(AUTOMATION_RUN_UPDATED_EVENT_NAME)).toBe(0);
  });
});
