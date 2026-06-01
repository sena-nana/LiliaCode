import { fireEvent, render } from "@testing-library/vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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

const singleAskWithOtherSpec: AskUserSpec = {
  title: "Claude 想确认一下",
  questions: [
    {
      id: "q-1",
      question: "选哪个方案？",
      mode: "single",
      allowOther: true,
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

const bashToolConsent: ToolConsentRequest = {
  ...toolConsent,
  requestId: "bash-tool-1",
  toolName: "Bash",
  input: { command: "pwd" },
};

const projectCwd = "D:\\PROJECT\\workspace\\Lilia";

function renderRunningComposer() {
  return render(ChatComposer, {
    props: {
      state: baseState,
      attachments: [],
      sending: true,
    },
  });
}

const scrollHeights: number[] = [];
let scrollHeightDescriptor: PropertyDescriptor | undefined;

async function flushContextSearch() {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

function placeEditableCaret(element: HTMLElement, offset: number) {
  const selection = window.getSelection();
  const range = document.createRange();
  const textNode = element.firstChild;
  if (textNode?.nodeType === Node.TEXT_NODE) {
    range.setStart(textNode, Math.min(offset, textNode.textContent?.length ?? 0));
  } else {
    range.selectNodeContents(element);
    range.collapse(false);
  }
  selection?.removeAllRanges();
  selection?.addRange(range);
}

async function setComposerText(view: ReturnType<typeof render>, text: string) {
  const input = view.getByRole("textbox") as HTMLElement;
  if (input instanceof HTMLTextAreaElement) {
    await fireEvent.update(input, text);
    return input;
  }
  input.textContent = text;
  placeEditableCaret(input, text.length);
  await fireEvent.input(input);
  return input;
}

function composerText(input: HTMLElement): string {
  return input instanceof HTMLTextAreaElement ? input.value : input.textContent ?? "";
}

beforeEach(() => {
  vi.useFakeTimers();
  scrollHeights.length = 0;

  const textareaProto = HTMLTextAreaElement.prototype;
  scrollHeightDescriptor = Object.getOwnPropertyDescriptor(textareaProto, "scrollHeight");
  Object.defineProperty(textareaProto, "scrollHeight", {
    configurable: true,
    get() {
      return scrollHeights.shift() ?? 30;
    },
  });
});

afterEach(() => {
  const textareaProto = HTMLTextAreaElement.prototype;
  if (scrollHeightDescriptor) {
    Object.defineProperty(textareaProto, "scrollHeight", scrollHeightDescriptor);
  } else {
    delete (textareaProto as { scrollHeight?: number }).scrollHeight;
  }
  vi.useRealTimers();
});

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

    await setComposerText(view, "补充上下文");
    await fireEvent.click(view.getByRole("button", { name: "加入调度队列" }));

    expect(view.emitted("send")?.[0]).toEqual(["补充上下文", []]);
    expect(view.emitted("interrupt")).toBeUndefined();
  });


  it("@ 搜索选中结果后转为上下文附件并清理 mention", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "参考 @read");
    await flushContextSearch();
    expect(view.getAllByText("README.md").length).toBeGreaterThan(0);

    await fireEvent.keyDown(input, { key: "Enter", code: "Enter" });

    expect(view.emitted("add-context-attachment")?.[0]?.[0]).toMatchObject({
      name: "README.md",
      path: "D:\\PROJECT\\workspace\\Lilia\\README.md",
      kind: "file",
    });
    expect(view.emitted("send")).toBeUndefined();
    expect(composerText(input)).toContain("参考");
    expect(composerText(input)).toContain("README.md");
    expect(composerText(input)).not.toContain("D:\\PROJECT\\workspace\\Lilia\\README.md");
    expect(composerText(input)).not.toContain("@read");
  });


  it("@ 搜索目录时 Tab 进入目录且不生成附件", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "@big");
    await flushContextSearch();
    await fireEvent.keyDown(input, { key: "Tab", code: "Tab" });
    await flushContextSearch();

    expect(view.emitted("add-context-attachment")).toBeUndefined();
    expect(composerText(input)).toBe("@big-dir/");
    expect(view.getByText("inside.md")).toBeInTheDocument();
  });


  it("@ 绝对路径不存在时提示且不创建附件", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "@D:\\PROJECT\\workspace\\Lilia\\missing.md");
    await flushContextSearch();

    expect(view.getByText(/路径不存在/)).toBeInTheDocument();
    await fireEvent.keyDown(input, { key: "Enter", code: "Enter" });

    expect(view.emitted("add-context-attachment")).toBeUndefined();
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


  it("pending AskUser 只有点击允许的其他选项后才显示输入框并返回 other", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        pendingAsk: pendingAsk(singleAskWithOtherSpec),
      },
    });

    expect(view.queryByRole("textbox")).toBeNull();
    await fireEvent.click(view.getByRole("radio", { name: "其他" }));
    const input = view.getByRole("textbox");
    const entryRow = input.closest(".chat-composer__entry-row");
    expect(entryRow).not.toBeNull();
    expect(entryRow).toContainElement(view.getByRole("button", { name: "完成" }));
    await fireEvent.update(input, "我想选第三种");
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


  it("pending 工具授权中输入文本后修改按钮会作为拒绝备注返回", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        toolConsent,
      },
    });

    expect(view.getByRole("button", { name: "忽略" })).toBeDisabled();
    await fireEvent.update(view.getByRole("textbox"), "先不要写这个文件");
    await fireEvent.click(view.getByRole("button", { name: "修改" }));

    expect(view.emitted("resolve-tool-consent")?.[0]).toEqual([
      "deny",
      "先不要写这个文件",
    ]);
  });


  it("pending Bash 授权同意时返回用户修改后的 updatedInput", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        toolConsent: bashToolConsent,
      },
    });

    await fireEvent.click(view.getByRole("button", { name: "编辑完整命令" }));
    await fireEvent.update(view.getByRole("textbox", { name: "编辑命令" }), "pwd && echo ok");
    await fireEvent.click(view.getByRole("button", { name: "同意" }));

    expect(view.emitted("resolve-tool-consent")?.[0]).toEqual([
      "allow",
      undefined,
      { command: "pwd && echo ok" },
    ]);
  });
});
