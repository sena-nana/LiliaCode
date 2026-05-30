import { fireEvent, render } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import type { AskUserSpec, ChatComposerState } from "@lilia/contracts";
import type { PendingAsk } from "../src/composables/useAskUser";
import type { ToolConsentRequest } from "../src/services/chat";
import ChatComposer from "../src/components/chat/ChatComposer.vue";

const baseState: ChatComposerState = {
  taskId: "task-1",
  backend: "claude",
  model: "claude-sonnet-4-6",
  planMode: false,
  permission: "full",
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

const singleAskSpec: AskUserSpec = {
  title: "Claude 想确认一下",
  questions: [
    {
      id: "q-1",
      question: "选哪个方案？",
      mode: "single",
      options: [
        { id: "a", label: "A" },
        { id: "b", label: "B" },
      ],
    },
  ],
};

const toolConsent: ToolConsentRequest = {
  taskId: "task-1",
  turnId: "turn-tool",
  backend: "claude",
  requestId: "tool-1",
  toolName: "Write",
  input: { file_path: "src/main.ts" },
  title: null,
  displayName: null,
  description: null,
  blockedPath: null,
  decisionReason: null,
  toolUseId: null,
};

function renderRunningComposer() {
  return render(ChatComposer, {
    props: {
      state: baseState,
      attachments: [],
      sending: true,
    },
  });
}

describe("ChatComposer", () => {
  it("Agent 运行且空输入时发送按钮切为打断", async () => {
    const view = renderRunningComposer();

    const interrupt = view.getByRole("button", { name: "打断 Agent" });
    expect(interrupt).not.toBeDisabled();

    await fireEvent.click(interrupt);

    expect(view.emitted("interrupt")).toHaveLength(1);
    expect(view.emitted("send")).toBeUndefined();
  });

  it("Agent 运行但有输入时仍发送到调度队列", async () => {
    const view = renderRunningComposer();

    await fireEvent.update(view.getByRole("textbox"), "补充上下文");
    await fireEvent.click(view.getByRole("button", { name: "加入调度队列" }));

    expect(view.emitted("send")?.[0]).toEqual(["补充上下文", []]);
    expect(view.emitted("interrupt")).toBeUndefined();
  });

  it("Agent 运行且空输入时 textarea Enter 不触发打断", async () => {
    const view = renderRunningComposer();

    await fireEvent.keyDown(view.getByRole("textbox"), {
      key: "Enter",
      code: "Enter",
    });

    expect(view.emitted("interrupt")).toBeUndefined();
    expect(view.emitted("send")).toBeUndefined();
  });

  it("计划模式独立于执行权限切换", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    const planButton = view.getByRole("button", { name: "开启计划模式" });
    expect(planButton).toHaveAttribute("aria-pressed", "false");

    await fireEvent.click(planButton);

    expect(view.emitted("update:state")?.[0]?.[0]).toMatchObject({
      planMode: true,
      permission: "full",
    });
  });

  it("pending 状态隐藏功能按钮，解除后恢复", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        pendingAsk: pendingAsk(singleAskSpec),
      },
    });

    expect(view.getByRole("region", { name: "Claude 想确认一下" }))
      .toHaveClass("composer-inline");
    expect(view.queryByRole("button", { name: "添加附件" })).toBeNull();
    expect(view.queryByRole("button", { name: "开启计划模式" })).toBeNull();

    await view.rerender({
      state: baseState,
      attachments: [],
      pendingAsk: null,
    });

    expect(view.getByRole("button", { name: "添加附件" })).toBeInTheDocument();
    expect(view.getByRole("button", { name: "开启计划模式" })).toBeInTheDocument();
  });

  it("pending 状态切换时复用同一个输入框节点", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    const input = view.getByRole("textbox");
    await fireEvent.update(input, "先保留这段输入");

    await view.rerender({
      state: baseState,
      attachments: [],
      pendingAsk: pendingAsk(singleAskSpec),
    });

    const pendingInput = view.getByRole("textbox");
    expect(pendingInput).toBe(input);
    expect(pendingInput).toHaveValue("");

    await fireEvent.update(pendingInput, "pending 回答");

    await view.rerender({
      state: baseState,
      attachments: [],
      pendingAsk: null,
    });

    const restoredInput = view.getByRole("textbox");
    expect(restoredInput).toBe(input);
    expect(restoredInput).toHaveValue("先保留这段输入");
  });

  it("pending AskUser 中输入文本后，完成按钮会作为 other 回答返回", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        pendingAsk: pendingAsk(singleAskSpec),
      },
    });

    expect(view.queryByRole("radio", { name: "其他..." })).toBeNull();
    await fireEvent.update(view.getByRole("textbox"), "我想选第三种");
    await fireEvent.click(view.getByRole("button", { name: "完成" }));

    expect(view.emitted("resolve-ask-user")?.[0]?.[0]).toEqual({
      cancelled: false,
      answers: {
        "q-1": {
          questionId: "q-1",
          value: "other",
          notes: "我想选第三种",
        },
      },
    });
  });

  it("pending 工具授权中发送文本会作为拒绝备注返回", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        toolConsent,
      },
    });

    await fireEvent.update(view.getByRole("textbox"), "先不要写这个文件");
    await fireEvent.click(view.getByRole("button", { name: "发送拒绝备注" }));

    expect(view.emitted("resolve-tool-consent")?.[0]).toEqual([
      "deny",
      "先不要写这个文件",
    ]);
  });

  it("pending 工具授权保留忽略和同意，不再显示拒绝按钮", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        toolConsent,
      },
    });

    expect(view.queryByRole("button", { name: "拒绝" })).toBeNull();
    expect(view.getByRole("button", { name: "忽略" })).toBeInTheDocument();

    await fireEvent.click(view.getByRole("button", { name: "同意" }));

    expect(view.emitted("resolve-tool-consent")?.[0]).toEqual(["allow", undefined]);
  });
});
