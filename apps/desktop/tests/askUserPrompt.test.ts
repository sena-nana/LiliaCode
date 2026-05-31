import { fireEvent, render } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import type { AskUserSpec, ChatComposerState } from "@lilia/contracts";
import type { PendingAsk } from "../src/composables/useAskUser";
import ChatComposer from "../src/components/chat/ChatComposer.vue";

const baseState: ChatComposerState = {
  taskId: "task-1",
  backend: "claude",
  model: "claude-sonnet-4-6",
  planMode: false,
  permission: "ask",
};

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

const recommendedSingleSpec: AskUserSpec = {
  title: "Lilia 想确认一下",
  questions: [
    {
      id: "q-recommended",
      header: "入口",
      question: "选一个入口。",
      mode: "single",
      options: [
        { id: "alpha", label: "Alpha", recommended: true },
        { id: "beta", label: "Beta" },
      ],
    },
  ],
};

const singleWithOtherSpec: AskUserSpec = {
  title: "Lilia 想确认一下",
  questions: [
    {
      id: "q-other",
      header: "入口",
      question: "选一个入口。",
      mode: "single",
      allowOther: true,
      options: [
        { id: "alpha", label: "Alpha" },
        { id: "beta", label: "Beta" },
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

function pendingAsk(spec: AskUserSpec): PendingAsk {
  return {
    id: 1,
    spec,
    taskId: "task-1",
    turnId: "turn-1",
    resolve: () => {},
  };
}

function renderComposer(spec: AskUserSpec) {
  return render(ChatComposer, {
    props: {
      state: baseState,
      attachments: [],
      pendingAsk: pendingAsk(spec),
    },
  });
}

function optionItem(button: HTMLElement): HTMLElement {
  const item = button.closest(".composer-inline__option");
  expect(item).not.toBeNull();
  return item as HTMLElement;
}

describe("ChatComposer AskUser pending prompt", () => {
  it("从问题 2 回到问题 1 后再前进，会保留问题 2 已选项", async () => {
    const view = renderComposer(multiStepSpec);

    await fireEvent.click(view.getByRole("radio", { name: "Alpha" }));
    expect(view.getByRole("radio", { name: "Alpha" }))
      .toHaveAttribute("aria-checked", "true");
    expect(view.getByRole("radio", { name: "Beta" }))
      .toHaveAttribute("aria-checked", "false");
    await fireEvent.mouseEnter(view.getByRole("radio", { name: "Beta" }));
    expect(view.getByRole("radio", { name: "Alpha" }))
      .toHaveAttribute("aria-checked", "true");
    expect(view.getByRole("radio", { name: "Beta" }))
      .toHaveAttribute("aria-checked", "false");
    await fireEvent.click(view.getByRole("button", { name: /继续/ }));
    await fireEvent.click(view.getByRole("checkbox", { name: "保留选择" }));

    expect(view.getByRole("checkbox", { name: "保留选择" }))
      .toHaveAttribute("aria-checked", "true");

    await fireEvent.click(view.getByRole("button", { name: "上一题" }));
    await fireEvent.click(view.getByRole("button", { name: /继续/ }));

    expect(view.getByRole("checkbox", { name: "保留选择" }))
      .toHaveAttribute("aria-checked", "true");
  });

  it("推荐单选项不默认高亮，鼠标离开后清除临时高亮", async () => {
    const view = renderComposer(recommendedSingleSpec);
    const alpha = view.getByRole("radio", { name: /Alpha/ });
    const beta = view.getByRole("radio", { name: "Beta" });
    const alphaItem = optionItem(alpha);
    const betaItem = optionItem(beta);

    expect(alphaItem).toHaveClass("is-recommended");
    expect(alphaItem).not.toHaveClass("is-active");
    expect(betaItem).not.toHaveClass("is-active");

    await fireEvent.mouseEnter(beta);
    expect(betaItem).toHaveClass("is-active");
    await fireEvent.mouseLeave(beta);
    expect(betaItem).not.toHaveClass("is-active");

    await fireEvent.mouseEnter(beta);
    await fireEvent.click(beta);
    expect(beta).toHaveAttribute("aria-checked", "true");
    expect(betaItem).toHaveClass("is-picked", "is-active");

    await fireEvent.mouseLeave(beta);
    expect(betaItem).not.toHaveClass("is-active");
    expect(betaItem).toHaveClass("is-picked");
    expect(beta).toHaveAttribute("aria-checked", "true");
  });

  it("允许 other 的单选题点击其他后才显示输入框", async () => {
    const view = renderComposer(singleWithOtherSpec);

    expect(view.queryByRole("textbox")).toBeNull();
    await fireEvent.click(view.getByRole("radio", { name: "其他" }));

    expect(view.getByRole("radio", { name: "其他" }))
      .toHaveAttribute("aria-checked", "true");
    expect(view.getByRole("textbox")).toBeInTheDocument();
  });

  it("未允许 other 的单选题不显示其他选项和输入框", () => {
    const view = renderComposer(recommendedSingleSpec);

    expect(view.queryByRole("radio", { name: "其他" })).toBeNull();
    expect(view.queryByRole("textbox")).toBeNull();
  });

  it("计划确认提示作为 composer 内部一行紧凑扩展", () => {
    const view = renderComposer(planApprovalSpec);

    const prompt = view.getByRole("region", { name: "确认 Claude 计划" });
    expect(prompt).toHaveClass("composer-inline", "composer-inline--plan");
    expect(view.queryByText(/确认后将按/)).toBeNull();
    expect(view.queryByRole("button", { name: "先不执行" })).toBeNull();
    expect(view.getByRole("button", { name: "忽略" })).toBeDisabled();
    expect(view.getByRole("button", { name: "同意" })).toBeInTheDocument();
  });
});
