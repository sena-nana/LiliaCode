import { fireEvent, render } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import type { AskUserSpec } from "@lilia/contracts";
import AskUserPrompt from "../src/components/chat/AskUserPrompt.vue";

const multiStepSpec: AskUserSpec = {
  title: "Lilia 想确认 2 件事",
  questions: [
    {
      id: "q-1",
      header: "第一题",
      question: "先选一个入口。",
      mode: "single",
      options: [
        { id: "alpha", label: "Alpha" },
        { id: "beta", label: "Beta" },
      ],
    },
    {
      id: "q-2",
      header: "第二题",
      question: "需要保留哪些状态？",
      mode: "multi",
      options: [
        { id: "keep", label: "保留选择" },
        { id: "skip", label: "忽略选择" },
      ],
    },
  ],
};

const planApprovalSpec: AskUserSpec = {
  title: "确认 Claude 计划",
  source: "Claude Plan",
  intent: "plan_approval",
  questions: [
    {
      id: "approve-plan",
      header: "计划确认",
      question: "",
      mode: "confirm",
      confirmLabel: "按计划执行",
      cancelLabel: "先不执行",
    },
  ],
};

describe("AskUserPrompt", () => {
  it("从问题 2 回到问题 1 后再前进，会保留问题 2 已选项", async () => {
    const view = render(AskUserPrompt, {
      props: {
        spec: multiStepSpec,
      },
    });

    await fireEvent.click(view.getByRole("radio", { name: "Alpha" }));
    await fireEvent.click(view.getByRole("checkbox", { name: "保留选择" }));

    expect(view.getByRole("checkbox", { name: "保留选择" }))
      .toHaveAttribute("aria-checked", "true");

    await fireEvent.click(view.getByRole("button", { name: "上一题" }));
    await fireEvent.click(view.getByRole("button", { name: /继续/ }));

    expect(view.getByRole("checkbox", { name: "保留选择" }))
      .toHaveAttribute("aria-checked", "true");
  });

  it("计划确认提示使用一行紧凑样式", () => {
    const view = render(AskUserPrompt, {
      props: {
        spec: planApprovalSpec,
      },
    });

    const prompt = view.getByRole("region", { name: "确认 Claude 计划" });
    expect(prompt).toHaveClass("ask-user--plan-approval");
    expect(view.queryByText(/确认后将按/)).toBeNull();
    expect(view.getByRole("button", { name: "按计划执行" })).toBeInTheDocument();
    expect(view.getByRole("button", { name: "先不执行" })).toBeInTheDocument();
  });
});
