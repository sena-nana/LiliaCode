import { fireEvent, render, within } from "@testing-library/vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AskUserSpec, ChatAttachment, ChatComposerState } from "@lilia/contracts";
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

const previewImageAttachment: ChatAttachment = {
  id: "att-img",
  name: "context-preview.png",
  path: "D:\\PROJECT\\workspace\\Lilia\\screenshots\\context-preview.png",
  kind: "file",
  size: 42,
  exists: true,
  mime: "image/png",
  directory: null,
};

const largeDirectoryAttachment: ChatAttachment = {
  id: "att-big",
  name: "big-dir",
  path: "D:\\PROJECT\\workspace\\Lilia\\big-dir",
  kind: "directory",
  size: null,
  exists: true,
  mime: null,
  directory: {
    fileCount: 250,
    directoryCount: 18,
    totalSize: 24 * 1024 * 1024,
    truncated: true,
    unreadableCount: 0,
  },
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

const scrollHeights: number[] = [];
let scrollHeightDescriptor: PropertyDescriptor | undefined;

function setMeasuredScrollHeight(scrollHeight: number) {
  scrollHeights.push(scrollHeight);
}

async function flushComposerResize() {
  await vi.advanceTimersByTimeAsync(16);
}

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

  it("@ 输入打开文件上下文搜索，Esc 关闭", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "@read");
    await flushContextSearch();

    expect(view.getByRole("listbox", { name: "文件上下文搜索" })).toBeInTheDocument();
    expect(view.getAllByText("README.md").length).toBeGreaterThan(0);

    await fireEvent.keyDown(input, { key: "Escape" });

    expect(view.queryByRole("listbox", { name: "文件上下文搜索" })).toBeNull();
  });

  it("@ 搜索面板不渲染额外搜索头", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "@read");
    await flushContextSearch();

    const panel = view.getByRole("listbox", { name: "文件上下文搜索" });
    expect(panel).toBeInTheDocument();
    expect(within(panel).queryByText("@read")).toBeNull();
  });

  it("@ 搜索结果将路径内联显示在 name 后且相同路径不重复", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "@read");
    await flushContextSearch();
    expect(view.getAllByText("README.md")).toHaveLength(1);

    await setComposerText(view, "@composer");
    await flushContextSearch();

    const main = view.getByText("ChatComposer.vue").closest(".chat-composer__context-main");
    expect(main).not.toBeNull();
    expect(within(main as HTMLElement).getByText("apps/desktop/src/components/chat/ChatComposer.vue"))
      .toBeInTheDocument();
    expect(main?.querySelector(".chat-composer__context-name")?.nextElementSibling)
      .toHaveClass("chat-composer__context-path");
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

  it("@ 可在输入内容中任意位置紧贴文本触发", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "参考@read");
    await flushContextSearch();
    expect(view.getAllByText("README.md").length).toBeGreaterThan(0);

    await fireEvent.keyDown(input, { key: "Enter", code: "Enter" });

    expect(view.emitted("add-context-attachment")?.[0]?.[0]).toMatchObject({
      name: "README.md",
      path: "D:\\PROJECT\\workspace\\Lilia\\README.md",
    });
    expect(composerText(input)).toContain("参考");
    expect(composerText(input)).toContain("README.md");
    expect(composerText(input)).not.toContain("@read");
  });

  it("@ 搜索目录时 Enter 选中文件夹生成附件", async () => {
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
    expect(view.getByText("big-dir")).toBeInTheDocument();

    await fireEvent.keyDown(input, { key: "Enter", code: "Enter" });

    expect(view.emitted("add-context-attachment")?.[0]?.[0]).toMatchObject({
      name: "big-dir",
      kind: "directory",
    });
    expect(composerText(input)).toContain("big-dir");
    expect(composerText(input)).not.toContain("@big");
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

  it("@ 搜索目录内按 Esc 回退到上一级，根目录再按 Esc 关闭", async () => {
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
    expect(composerText(input)).toBe("@big-dir/");

    await fireEvent.keyDown(input, { key: "Escape", code: "Escape" });
    await flushContextSearch();
    expect(composerText(input)).toBe("@");
    expect(view.getByRole("listbox", { name: "文件上下文搜索" })).toBeInTheDocument();

    await fireEvent.keyDown(input, { key: "Escape", code: "Escape" });
    expect(view.queryByRole("listbox", { name: "文件上下文搜索" })).toBeNull();
  });

  it("单独 @ 按 Enter 作为普通文本发送，不默认选中文件", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "@");
    await flushContextSearch();
    expect(view.getByRole("listbox", { name: "文件上下文搜索" })).toBeInTheDocument();

    await fireEvent.keyDown(input, { key: "Enter", code: "Enter" });

    expect(view.emitted("add-context-attachment")).toBeUndefined();
    expect(view.emitted("send")?.[0]).toEqual(["@", []]);
  });

  it("@ 搜索文件时 Tab 不选中附件", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "@read");
    await flushContextSearch();
    await fireEvent.keyDown(input, { key: "Tab", code: "Tab" });

    expect(view.emitted("add-context-attachment")).toBeUndefined();
    expect(composerText(input)).toBe("@read");
  });

  it("@ 无匹配后继续输入会退出搜索，删回单独 @ 后重新打开", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });

    await setComposerText(view, "@zzzz");
    await flushContextSearch();
    expect(view.getByText("没有匹配的文件或目录")).toBeInTheDocument();

    await setComposerText(view, "@zzzzz");
    await flushContextSearch();
    expect(view.queryByRole("listbox", { name: "文件上下文搜索" })).toBeNull();

    await setComposerText(view, "@");
    await flushContextSearch();
    expect(view.getByRole("listbox", { name: "文件上下文搜索" })).toBeInTheDocument();
    expect(view.getByText("README.md")).toBeInTheDocument();
  });

  it("@ 搜索默认隐藏点文件，输入点后显示", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "@env");
    await flushContextSearch();
    expect(view.queryByText(".env")).toBeNull();

    await setComposerText(view, "@.");
    await flushContextSearch();
    expect(view.getByText(".env")).toBeInTheDocument();
  });

  it("@ 搜索默认隐藏 gitignore 忽略项，明确路径后显示", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "@dist");
    await flushContextSearch();
    expect(view.queryByText("dist")).toBeNull();

    await setComposerText(view, "@dist/");
    await flushContextSearch();
    expect(view.getByText("app.js")).toBeInTheDocument();
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

  it("@ 绝对路径允许空格并可转为附件", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "@D:\\PROJECT\\workspace\\My Project\\context.png");
    await flushContextSearch();
    expect(view.getByText("context.png")).toBeInTheDocument();

    await fireEvent.keyDown(input, { key: "Enter", code: "Enter" });

    expect(view.emitted("add-context-attachment")?.[0]?.[0]).toMatchObject({
      name: "context.png",
      path: "D:\\PROJECT\\workspace\\My Project\\context.png",
      kind: "file",
      mime: "image/png",
    });
    expect(composerText(input)).toContain("context.png");
    expect(composerText(input)).not.toContain("@D:\\PROJECT");
  });

  it("pending 输入态不启用 @ 文件上下文搜索", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
        toolConsent,
      },
    });

    await fireEvent.update(view.getByRole("textbox"), "@read");
    await flushContextSearch();

    expect(view.queryByRole("listbox", { name: "文件上下文搜索" })).toBeNull();
  });

  it("行内引用卡显示目录体量提示，图片额外显示预览", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [previewImageAttachment, largeDirectoryAttachment],
      },
    });
    await flushContextSearch();

    const thumb = view.container.querySelector(".chat-attachment-chip__thumb");
    expect(thumb).not.toBeNull();
    expect(thumb as Element).toHaveAttribute(
      "src",
      "asset://D:/PROJECT/workspace/Lilia/screenshots/context-preview.png",
    );
    expect(view.getByText("目录较大")).toBeInTheDocument();
    expect(view.getAllByText("context-preview.png").length).toBeGreaterThan(0);
  });

  it("移除行内引用卡会同步移除附件", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [previewImageAttachment],
      },
    });
    await flushContextSearch();

    await fireEvent.click(view.getByRole("button", {
      name: "移除文件引用 context-preview.png",
    }));

    expect(view.emitted("remove-attachment")?.[0]).toEqual(["att-img"]);
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

  it("需要输入的 pending 状态切换时不污染普通输入内容", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    const input = await setComposerText(view, "先保留这段输入");

    await view.rerender({
      state: baseState,
      attachments: [],
      toolConsent,
    });

    const pendingInput = view.getByRole("textbox");
    expect(pendingInput).not.toBe(input);
    expect(pendingInput).toHaveValue("");

    await fireEvent.update(pendingInput, "pending 回答");

    await view.rerender({
      state: baseState,
      attachments: [],
      pendingAsk: null,
      toolConsent: null,
    });

    const restoredInput = view.getByRole("textbox");
    expect(restoredInput).not.toBe(pendingInput);
    expect(composerText(restoredInput as HTMLElement)).toContain("先保留这段输入");
  });

  it("pending textarea 输入超过一行时向上扩展，最多三行后滚动", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        toolConsent,
      },
    });
    const input = view.getByRole("textbox") as HTMLTextAreaElement;

    setMeasuredScrollHeight(30);
    await fireEvent.update(input, "一行");
    await flushComposerResize();
    expect(input.style.height).toBe("30px");
    expect(input.style.overflowY).toBe("hidden");

    setMeasuredScrollHeight(52);
    await fireEvent.update(input, "第一行\n第二行");
    await flushComposerResize();
    expect(input.style.height).toBe("52px");
    expect(input.style.overflowY).toBe("hidden");

    setMeasuredScrollHeight(96);
    input.scrollTop = 22;
    await fireEvent.update(input, "第一行\n第二行\n第三行\n第四行");
    await flushComposerResize();
    expect(input.style.height).toBe("74px");
    expect(input.style.overflowY).toBe("hidden");
    expect(input.scrollTop).toBe(0);
    await vi.advanceTimersByTimeAsync(160);
    expect(input.style.overflowY).toBe("auto");
    expect(input.scrollTop).toBe(96);

    input.scrollTop = 22;
    setMeasuredScrollHeight(30);
    await fireEvent.update(input, "缩回一行");
    await flushComposerResize();
    expect(input.style.height).toBe("30px");
    expect(input.style.overflowY).toBe("hidden");
    expect(input.scrollTop).toBe(0);
  });

  it("发送后 rich 输入内容清空", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    const input = await setComposerText(view, "第一行\n第二行\n第三行");
    await fireEvent.click(view.getByRole("button", { name: "发送" }));

    expect(view.emitted("send")?.[0]).toEqual(["第一行\n第二行\n第三行", []]);
    expect(composerText(input)).toBe("");
  });

  it("普通 rich 输入支持 Shift+Enter 插入换行且不发送", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    const input = await setComposerText(view, "第一行");
    await fireEvent.keyDown(input, { key: "Enter", code: "Enter", shiftKey: true });

    expect(view.emitted("send")).toBeUndefined();
    expect(composerText(input)).toBe("第一行\n");

    input.textContent = `${composerText(input)}第二行`;
    placeEditableCaret(input, "第一行\n第二行".length);
    await fireEvent.input(input);
    await fireEvent.click(view.getByRole("button", { name: "发送" }));

    expect(view.emitted("send")?.[0]).toEqual(["第一行\n第二行", []]);
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

  it("pending AskUser 不允许 other 时不显示自定义输入", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        pendingAsk: pendingAsk(singleAskSpec),
      },
    });

    expect(view.queryByRole("radio", { name: "其他" })).toBeNull();
    expect(view.queryByRole("textbox")).toBeNull();
    expect(view.getByRole("button", { name: "完成" })).toBeDisabled();
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

  it("pending 工具授权保留忽略和同意，不再显示拒绝按钮", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        toolConsent,
      },
    });

    expect(view.queryByRole("button", { name: "拒绝" })).toBeNull();
    expect(view.getByRole("button", { name: "忽略" })).toBeDisabled();

    await fireEvent.click(view.getByRole("button", { name: "同意" }));

    expect(view.emitted("resolve-tool-consent")?.[0]).toEqual(["allow", undefined]);
  });

  it("pending Bash 授权显示完整命令框，点击可编辑且取消会重置草稿", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        toolConsent: bashToolConsent,
      },
    });

    expect(view.getByText("COMMAND")).toBeInTheDocument();
    expect(view.getByRole("button", { name: "编辑完整命令" })).toHaveTextContent("pwd");

    await fireEvent.click(view.getByRole("button", { name: "编辑完整命令" }));
    expect(view.getByRole("textbox", { name: "编辑命令" })).toHaveValue("pwd");
    expect(view.getByText("正在编辑命令。")).toBeInTheDocument();
    expect(view.getByRole("button", { name: "取消" })).toBeEnabled();

    await fireEvent.update(view.getByRole("textbox", { name: "编辑命令" }), "pwd && echo ok");
    await fireEvent.click(view.getByRole("button", { name: "取消" }));

    expect(view.queryByRole("textbox", { name: "编辑命令" })).toBeNull();
    expect(view.queryByText("正在编辑命令。")).toBeNull();
    expect(view.getByRole("button", { name: "编辑完整命令" })).toHaveTextContent("pwd");
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

  it("pending Bash 授权编辑为空命令时不能同意", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        toolConsent: bashToolConsent,
      },
    });

    await fireEvent.click(view.getByRole("button", { name: "编辑完整命令" }));
    await fireEvent.update(view.getByRole("textbox", { name: "编辑命令" }), "   ");

    expect(view.getByRole("button", { name: "同意" })).toBeDisabled();
  });
});
