import { describe, expect, it } from "vitest";
import {
  clampPercent,
  formatPercent,
  quotaPercentTone,
  quotaWindowLabel,
} from "../src/utils/quotaDisplay";

describe("quota display helpers", () => {
  it("clamps percentages for display and ring progress", () => {
    expect(clampPercent(-12)).toBe(0);
    expect(clampPercent(42.4)).toBe(42.4);
    expect(clampPercent(142)).toBe(100);
    expect(formatPercent(142)).toBe("100%");
  });

  it("maps quota ring tone by used percentage", () => {
    expect(quotaPercentTone(60)).toBe("normal");
    expect(quotaPercentTone(85)).toBe("warn");
    expect(quotaPercentTone(100)).toBe("error");
  });

  it("formats quota window labels with remaining percentage", () => {
    expect(quotaWindowLabel(null)).toBe("暂无数据");
    expect(quotaWindowLabel({
      usedPercent: 25,
      windowDurationMins: 300,
      resetsAt: null,
    })).toBe("5 小时窗口 · 剩余 75%");
    expect(quotaWindowLabel({
      usedPercent: 90,
      windowDurationMins: 10080,
      resetsAt: null,
    })).toBe("7 天窗口 · 剩余 10%");
  });
});
