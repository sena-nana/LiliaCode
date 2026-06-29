import { describe, expect, it } from "vitest";
import {
  PROJECT_DASHBOARD_LIST_COMMAND,
  PROJECT_LIST_COMMAND,
  QUOTA_USAGE_CONSUME_CODEX_RATE_LIMIT_RESET_CREDIT_COMMAND,
  QUOTA_USAGE_GET_CODEX_ACCOUNT_STATUS_COMMAND,
  QUOTA_USAGE_GET_STATS_COMMAND,
  TASK_LIST_COMMAND,
  type CodexAccountQuotaStatus,
  type CodexRateLimitResetCreditConsumeResult,
  type QuotaUsageStats,
} from "@lilia/contracts";
import { getCurrentWindow, invoke, listen } from "../src/tauri/devMock";

const DEV_MOCK_TEST_EVENT_NAME = "mock";

describe("dev Tauri mock", () => {
  it("provides the minimum browser-preview shell data", async () => {
    await expect(invoke(PROJECT_LIST_COMMAND)).resolves.toEqual(
      expect.arrayContaining([expect.objectContaining({ id: "lilia" })]),
    );
    await expect(invoke(PROJECT_DASHBOARD_LIST_COMMAND)).resolves.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: "lilia",
          statusCounts: expect.objectContaining({ running: 1 }),
          totalTokens: expect.any(Number),
        }),
      ]),
    );
    await expect(invoke(TASK_LIST_COMMAND, { projectId: null })).resolves.toEqual(
      expect.arrayContaining([expect.objectContaining({ id: "o-001" })]),
    );
    await expect(listen(DEV_MOCK_TEST_EVENT_NAME, () => undefined)).resolves.toEqual(
      expect.any(Function),
    );
    expect(getCurrentWindow().label).toBe("main");
  });

  it("provides a usable quota usage and Codex account quota loop", async () => {
    const stats = await invoke<QuotaUsageStats>(QUOTA_USAGE_GET_STATS_COMMAND, {
      input: { days: 30, backend: "codex" },
    });

    expect(stats).toMatchObject({ days: 30, backend: "codex" });
    expect(stats.daily).toHaveLength(30);
    expect(stats.totals.totalTokens).toBeGreaterThan(0);
    expect(stats.backends).toEqual([
      expect.objectContaining({ backend: "codex", totalTokens: expect.any(Number) }),
    ]);
    expect(stats.projects).toEqual([
      expect.objectContaining({ projectId: "lilia", totalTokens: stats.totals.totalTokens }),
    ]);

    const status = await invoke<CodexAccountQuotaStatus>(
      QUOTA_USAGE_GET_CODEX_ACCOUNT_STATUS_COMMAND,
    );
    expect(status.available).toBe(true);
    expect(status.accountUsage?.summary.lifetimeTokens).toBeGreaterThan(0);
    expect(status.accountUsage?.dailyUsageBuckets?.length).toBeGreaterThanOrEqual(53);
    expect(status.accountUsage?.dailyUsageBuckets?.[0]).toEqual({
      startDate: expect.stringMatching(/^\d{4}-\d{2}-\d{2}$/),
      tokens: expect.any(Number),
    });
    expect(status.rateLimitResetCredits?.availableCount).toBe(2);

    const consumed = await invoke<CodexRateLimitResetCreditConsumeResult>(
      QUOTA_USAGE_CONSUME_CODEX_RATE_LIMIT_RESET_CREDIT_COMMAND,
      { input: { idempotencyKey: "dev-mock-test" } },
    );
    expect(consumed.outcome).toBe("reset");
    expect(consumed.status.rateLimitResetCredits?.availableCount).toBe(1);

    const consumedAgain = await invoke<CodexRateLimitResetCreditConsumeResult>(
      QUOTA_USAGE_CONSUME_CODEX_RATE_LIMIT_RESET_CREDIT_COMMAND,
      { input: { idempotencyKey: "dev-mock-test-2" } },
    );
    expect(consumedAgain.outcome).toBe("reset");
    expect(consumedAgain.status.rateLimitResetCredits?.availableCount).toBe(0);

    const consumedWithoutCredit = await invoke<CodexRateLimitResetCreditConsumeResult>(
      QUOTA_USAGE_CONSUME_CODEX_RATE_LIMIT_RESET_CREDIT_COMMAND,
      { input: { idempotencyKey: "dev-mock-test-3" } },
    );
    expect(consumedWithoutCredit.outcome).toBe("noCredit");
    expect(consumedWithoutCredit.status.rateLimitResetCredits?.availableCount).toBe(0);
  });
});

