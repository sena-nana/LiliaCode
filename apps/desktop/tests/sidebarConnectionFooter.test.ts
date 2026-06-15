import { fireEvent, render, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { describe, expect, it } from "vitest";
import SidebarConnectionFooter from "../src/components/sidebar/SidebarConnectionFooter.vue";
import { useConnectionStatus } from "../src/composables/useConnectionStatus";
import { createLiliaRouter } from "../src/router";
import {
  mockInvoke,
  setMockActiveBackend,
  setMockCodexAccountQuotaStatus,
  setMockRouterMode,
} from "./tauriMock";

async function renderFooter(initialRoute = "/") {
  const router = createLiliaRouter(createMemoryHistory());
  await router.push(initialRoute);
  await router.isReady();
  await useConnectionStatus({ probe: false }).refresh(false);

  return render(SidebarConnectionFooter, {
    global: {
      plugins: [router],
    },
  });
}

function providerBadge(container: HTMLElement): HTMLElement {
  const badge = container.querySelector(".sb-conn");
  if (!(badge instanceof HTMLElement)) {
    throw new Error("未找到 provider badge");
  }
  return badge;
}

function officialQuota(overrides: Record<string, unknown> = {}) {
  return {
    available: true,
    connectionMode: "codex-account",
    limitId: "codex",
    limitName: "Codex",
    planType: "Pro",
    rateLimitReachedType: null,
    fiveHour: {
      usedPercent: 42,
      windowDurationMins: 300,
      resetsAt: 1_800_000_000,
    },
    weekly: {
      usedPercent: 91,
      windowDurationMins: 10080,
      resetsAt: 1_800_300_000,
    },
    sparkFiveHour: {
      usedPercent: 12,
      windowDurationMins: 300,
      resetsAt: 1_800_060_000,
    },
    sparkWeekly: {
      usedPercent: 80,
      windowDurationMins: 10080,
      resetsAt: 1_800_360_000,
    },
    fetchedAt: 1_800_000_000_000,
    error: null,
    ...overrides,
  };
}

describe("SidebarConnectionFooter provider quota badge", () => {
  it("does not show quota rings for Claude", async () => {
    setMockActiveBackend("claude");
    const view = await renderFooter();

    await waitFor(() => {
      expect(providerBadge(view.container)).toHaveAttribute(
        "aria-label",
        "Claude API 未配置。点击进入设置。",
      );
    });
    expect(view.container.querySelector(".sb-quota-ring")).not.toBeInTheDocument();
  });

  it("does not show quota rings when Codex uses API mode", async () => {
    setMockActiveBackend("codex");
    setMockRouterMode("codex", "api");
    const view = await renderFooter();

    await waitFor(() => {
      expect(providerBadge(view.container)).toHaveAttribute(
        "aria-label",
        "Codex API 未配置。点击进入设置。",
      );
    });
    expect(view.container.querySelector(".sb-quota-ring")).not.toBeInTheDocument();
  });

  it("shows Codex official account quota rings and hover details", async () => {
    setMockActiveBackend("codex");
    setMockRouterMode("codex", "codex-account");
    setMockCodexAccountQuotaStatus(officialQuota());

    const view = await renderFooter();
    const provider = providerBadge(view.container);

    await waitFor(() => {
      expect(view.container.querySelectorAll(".sb-quota-ring")).toHaveLength(2);
    });
    const rings = Array.from(view.container.querySelectorAll<HTMLElement>(".sb-quota-ring"));
    expect(rings[0]).toHaveStyle({ "--quota-progress": "58" });
    expect(rings[1]).toHaveStyle({ "--quota-progress": "9" });

    await fireEvent.mouseEnter(provider);

    await view.findByRole("tooltip");
    expect(view.queryByText("Codex 官方账号")).not.toBeInTheDocument();
    expect(view.queryByText("Codex 官方账号额度")).not.toBeInTheDocument();
    expect(view.queryByText("Pro")).not.toBeInTheDocument();
    expect(view.queryByText("查询 01/15 16:00")).not.toBeInTheDocument();
    expect(view.queryByText("5 小时额度")).not.toBeInTheDocument();
    expect(view.queryByText("周额度")).not.toBeInTheDocument();
    expect(view.getByText("5h · 剩余 58%")).toBeInTheDocument();
    expect(view.getByText("7d · 剩余 9%")).toBeInTheDocument();
    expect(view.queryByText("Spark额度")).not.toBeInTheDocument();
    expect(view.getByText("5h · 剩余 88% · Spark")).toBeInTheDocument();
    expect(view.getByText("7d · 剩余 20% · Spark")).toBeInTheDocument();
    expect(view.getByText("刷新 01/15 16:00")).toBeInTheDocument();
    expect(view.getByText("刷新 01/19 03:20")).toBeInTheDocument();
    expect(view.getByText("刷新 01/16 08:40")).toBeInTheDocument();
    expect(view.getByText("刷新 01/19 20:00")).toBeInTheDocument();
    const meters = Array.from(view.container.querySelectorAll<HTMLElement>(".sb-conn-popover__quota-meter"));
    expect(meters).toHaveLength(4);
    expect(meters[0]).toHaveStyle({ "--quota-progress": "58" });
    expect(meters[1]).toHaveStyle({ "--quota-progress": "9" });
    expect(meters[2]).toHaveStyle({ "--quota-progress": "88" });
    expect(meters[3]).toHaveStyle({ "--quota-progress": "20" });
  });

  it("keeps the quota popover error state when the official quota call fails", async () => {
    setMockActiveBackend("codex");
    setMockRouterMode("codex", "codex-account");
    setMockCodexAccountQuotaStatus(officialQuota({
      available: false,
      fiveHour: null,
      weekly: null,
      sparkFiveHour: null,
      sparkWeekly: null,
      error: "Codex 官方额度接口未返回可识别的额度数据。",
    }));

    const view = await renderFooter();
    const provider = providerBadge(view.container);

    await fireEvent.mouseEnter(provider);

    expect(await view.findByRole("tooltip")).toHaveTextContent(
      "Codex 官方额度接口未返回可识别的额度数据。",
    );
    expect(view.container.querySelector(".sb-quota-ring")).not.toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("quota_usage_get_codex_account_status", {}, undefined);
  });
});
