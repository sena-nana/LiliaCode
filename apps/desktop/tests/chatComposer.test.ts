import { fireEvent, render, waitFor } from "@testing-library/vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AskUserSpec, ChatAttachment, ChatComposerState } from "@lilia/contracts";
import type { PendingAsk } from "../src/composables/useAskUser";
import type { ToolConsentRequest } from "../src/services/chat";
import ChatComposer from "../src/components/chat/ChatComposer.vue";
import { mockInvoke, setMockClipboardFilePaths } from "./tauriMock";

const baseState: ChatComposerState = {
  taskId: "task-1",
  backend: "claude",
  model: "claude-sonnet-4-6",
  planMode: false,
  goalMode: false,
  permission: "full",
};

const codexState: ChatComposerState = {
  ...baseState,
  backend: "codex",
  model: "gpt-5.5",
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

const codexCommandToolConsent: ToolConsentRequest = {
  ...toolConsent,
  backend: "codex",
  requestId: "codex-command-tool-1",
  toolName: "item/commandExecution/requestApproval",
  input: { command: "yarn test" },
  title: "Codex command approval",
  displayName: "item/commandExecution/requestApproval",
  description: "yarn test",
  toolUseId: "codex-approval-1",
  availableDecisions: ["accept", "decline", "cancel"],
  cwd: "D:/PROJECT/workspace/Lilia",
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

async function flushPasteTasks() {
  await Promise.resolve();
  await Promise.resolve();
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

async function openComposerActionMenu(view: ReturnType<typeof render>) {
  await fireEvent.click(view.getByRole("button", { name: "更多输入操作" }));
}

function createTextPasteEvent(text: string, html = ""): ClipboardEvent {
  const event = new Event("paste", { bubbles: true, cancelable: true }) as ClipboardEvent;
  Object.defineProperty(event, "clipboardData", {
    configurable: true,
    value: {
      files: [],
      items: [],
      getData: (type: string) => {
        if (type === "text/plain") return text;
        if (type === "text/html") return html;
        return "";
      },
    },
  });
  return event;
}

function createFilePasteEvent(files: File[]): ClipboardEvent {
  const event = new Event("paste", { bubbles: true, cancelable: true }) as ClipboardEvent;
  Object.defineProperty(event, "clipboardData", {
    configurable: true,
    value: {
      files,
      items: files.map((file) => ({
        kind: "file",
        type: file.type,
        getAsFile: () => file,
      })),
      getData: (type: string) => type === "text/plain" ? "" : "",
    },
  });
  return event;
}

function createPasteFile(input: {
  name: string;
  type?: string;
  bytes?: number[];
}): File {
  const bytes = new Uint8Array(input.bytes ?? []);
  return {
    name: input.name,
    type: input.type ?? "",
    arrayBuffer: async () => bytes.buffer.slice(0),
  } as File;
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

    expect(view.emitted("send")?.[0]).toEqual(["补充上下文", [], []]);
    expect(view.emitted("interrupt")).toBeUndefined();
  });


  it("主输入框只有空格时隐藏 placeholder 且不能发送", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    const input = await setComposerText(view, "   ");

    expect(input).not.toHaveClass("is-empty");
    expect(view.getByRole("button", { name: "发送" })).toBeDisabled();

    await fireEvent.click(view.getByRole("button", { name: "发送" }));

    expect(view.emitted("send")).toBeUndefined();
  });


  it("主输入框清空后只有浏览器填充 br 时显示 placeholder", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    const input = await setComposerText(view, "写点东西");
    input.replaceChildren(document.createElement("br"));
    placeEditableCaret(input, 0);
    await fireEvent.input(input);

    expect(input).toHaveClass("is-empty");
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

  it("@ 普通无结果继续输入会自动收起搜索面板", async () => {
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
    await waitFor(() => {
      expect(view.queryByRole("listbox", { name: "文件上下文搜索" })).toBeNull();
    });
  });

  it("@ 无结果自动收起后删回裸 @ 会重新打开搜索面板", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });

    await setComposerText(view, "@zzzz");
    await flushContextSearch();
    await setComposerText(view, "@zzzzz");
    await waitFor(() => {
      expect(view.queryByRole("listbox", { name: "文件上下文搜索" })).toBeNull();
    });
    await setComposerText(view, "@");
    await flushContextSearch();
    expect(view.getByRole("listbox", { name: "文件上下文搜索" })).toBeInTheDocument();
  });

  it("@ 路径型无结果继续输入时不会自动收起搜索面板", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });

    await setComposerText(view, "@dist/not-there");
    await flushContextSearch();
    expect(view.getByText("没有匹配的文件或目录")).toBeInTheDocument();

    await setComposerText(view, "@dist/not-there-more");
    await flushContextSearch();
    expect(view.getByRole("listbox", { name: "文件上下文搜索" })).toBeInTheDocument();
  });

  it("输入 / 时展示斜杠命令并可选择执行", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "/he");
    await flushContextSearch();

    expect(view.getByRole("listbox", { name: "斜杠命令" })).toBeInTheDocument();
    expect(view.getByText("/help")).toBeInTheDocument();

    await fireEvent.keyDown(input, { key: "Enter", code: "Enter" });

    expect(view.emitted("execute-slash-command")?.[0]?.[0]).toEqual({
      type: "slash_command",
      commandId: "native:help",
      source: "native",
      arguments: {},
    });
    expect(view.emitted("send")).toBeUndefined();
    expect(composerText(input)).toBe("");
  });

  it("斜杠命令不会影响 @ 文件上下文选择", async () => {
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
    await fireEvent.keyDown(input, { key: "Enter", code: "Enter" });

    expect(view.emitted("add-context-attachment")?.[0]?.[0]).toMatchObject({
      name: "README.md",
    });
    expect(view.emitted("execute-slash-command")).toBeUndefined();
  });

  it("文本粘贴会转成纯文本并去除富文本样式", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });
    const input = await setComposerText(view, "前  后");
    placeEditableCaret(input, 1);
    const event = createTextPasteEvent(
      "粗体文本",
      '<span style="color: red"><strong>粗体文本</strong></span>',
    );

    input.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(composerText(input)).toBe("前粗体文本  后");
    expect(input.querySelector("span, strong")).toBeNull();
    expect(mockInvoke).not.toHaveBeenCalledWith("chat_read_clipboard_file_paths", expect.anything());
    expect(mockInvoke).not.toHaveBeenCalledWith("chat_save_clipboard_image", expect.anything());
    expect(mockInvoke).not.toHaveBeenCalledWith("chat_save_clipboard_text", expect.anything());
  });

  it("未达到阈值的长文本粘贴仍直接插入输入框", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });
    const input = await setComposerText(view, "前  后");
    placeEditableCaret(input, 1);
    const text = "a".repeat(1999);
    const event = createTextPasteEvent(text);

    input.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(composerText(input)).toBe(`前${text}  后`);
    expect(mockInvoke).not.toHaveBeenCalledWith("chat_save_clipboard_text", expect.anything());
    expect(view.emitted("add-context-attachment")).toBeUndefined();
  });

  it("达到阈值的长文本粘贴会保存为临时文本上下文", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });
    const input = await setComposerText(view, "参考  继续");
    placeEditableCaret(input, 3);
    const text = "b".repeat(2000);
    const event = createTextPasteEvent(text);

    input.dispatchEvent(event);
    await flushPasteTasks();

    expect(event.defaultPrevented).toBe(true);
    expect(mockInvoke).toHaveBeenCalledWith("chat_save_clipboard_text", {
      input: { text },
    }, undefined);
    await waitFor(() => {
      expect(view.emitted("add-context-attachment")?.length).toBe(1);
    });
    expect(view.emitted("add-context-attachment")?.[0]?.[0]).toMatchObject({
      name: "粘贴文本 1.txt",
      path: "C:\\Users\\mock\\.lilia\\cache\\clipboard-texts\\clipboard-1.txt",
      kind: "file",
      mime: null,
    });
    expect(composerText(input)).toContain("参考");
    expect(composerText(input)).toContain("粘贴文本 1.txt");
    expect(composerText(input)).toContain("继续");
    expect(composerText(input)).not.toContain(text);
  });

  it("粘贴文件和文件夹时按当前光标插入上下文引用", async () => {
    setMockClipboardFilePaths([
      "D:\\PROJECT\\workspace\\Lilia\\README.md",
      "D:\\PROJECT\\workspace\\Lilia\\big-dir",
    ]);
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });
    const input = await setComposerText(view, "参考  继续");
    placeEditableCaret(input, 3);
    const event = createFilePasteEvent([createPasteFile({ name: "README.md" })]);

    input.dispatchEvent(event);
    await waitFor(() => {
      expect(view.emitted("add-context-attachment")?.length).toBe(2);
    });

    expect(event.defaultPrevented).toBe(true);
    expect(view.emitted("add-context-attachment")?.map(([attachment]) => attachment)).toEqual([
      expect.objectContaining({
        name: "README.md",
        path: "D:\\PROJECT\\workspace\\Lilia\\README.md",
        kind: "file",
      }),
      expect.objectContaining({
        name: "big-dir",
        path: "D:\\PROJECT\\workspace\\Lilia\\big-dir",
        kind: "directory",
      }),
    ]);
    expect(composerText(input)).toContain("参考");
    expect(composerText(input)).toContain("README.md");
    expect(composerText(input)).toContain("big-dir");
    expect(composerText(input)).toContain("继续");
  });

  it("粘贴剪贴板图片时保存为临时图片上下文", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });
    const input = await setComposerText(view, "看图 ");
    const event = createFilePasteEvent([createPasteFile({
      name: "screenshot.png",
      type: "image/png",
      bytes: [105, 109, 97, 103, 101],
    })]);

    input.dispatchEvent(event);
    await flushPasteTasks();

    expect(event.defaultPrevented).toBe(true);
    expect(mockInvoke).toHaveBeenCalledWith("chat_save_clipboard_image", {
      input: expect.objectContaining({
        mime: "image/png",
        bytesBase64: expect.any(String),
      }),
    }, undefined);
    await waitFor(() => {
      expect(view.emitted("add-context-attachment")?.length).toBe(1);
    });
    expect(view.emitted("add-context-attachment")?.[0]?.[0]).toMatchObject({
      name: "图片 1.png",
      path: "C:\\Users\\mock\\.lilia\\cache\\clipboard-images\\clipboard-1.png",
      kind: "file",
      mime: "image/png",
    });
    expect(composerText(input)).toContain("图片 1.png");

    const pastedAttachment = view.emitted("add-context-attachment")?.[0]?.[0] as ChatAttachment;
    await view.rerender({
      state: baseState,
      attachments: [pastedAttachment],
    });
    expect(view.getByLabelText("图片预览").querySelector("img")).toHaveAttribute(
      "src",
      "asset://C:/Users/mock/.lilia/cache/clipboard-images/clipboard-1.png",
    );
  });

  it("下方图片预览只显示缩略图", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [
          {
            id: "image-1",
            name: "图片 1.png",
            path: "D:\\PROJECT\\workspace\\Lilia\\shot.png",
            kind: "file",
            size: 42,
            exists: true,
            mime: "image/png",
            directory: null,
          },
        ],
      },
    });

    const preview = view.getByLabelText("图片预览");
    expect(preview.querySelector(".chat-attachment-chip--image-preview")).not.toBeNull();
    expect(preview.querySelector(".chat-attachment-chip__thumb")).not.toBeNull();
    expect(preview.querySelector(".chat-attachment-chip__name")).toBeNull();
    expect(preview.querySelector(".chat-attachment-chip__remove")).toBeNull();

    await fireEvent.click(view.getByRole("button", { name: "查看图片 图片 1.png" }));

    expect(view.emitted("open-image")?.[0]?.[0]).toMatchObject({
      src: "asset://D:/PROJECT/workspace/Lilia/shot.png",
      name: "图片 1.png",
      path: "D:\\PROJECT\\workspace\\Lilia\\shot.png",
      mime: "image/png",
      size: 42,
    });
  });

  it("重复粘贴同一路径不会重复插入", async () => {
    setMockClipboardFilePaths(["D:\\PROJECT\\workspace\\Lilia\\README.md"]);
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    input.dispatchEvent(createFilePasteEvent([createPasteFile({ name: "README.md" })]));
    await waitFor(() => {
      expect(view.emitted("add-context-attachment")?.length).toBe(1);
    });
    input.dispatchEvent(createFilePasteEvent([createPasteFile({ name: "README.md" })]));
    await Promise.resolve();

    const attachments = view.emitted("add-context-attachment") ?? [];
    expect(attachments).toHaveLength(1);
    expect(composerText(input).match(/README\.md/g)).toHaveLength(1);
  });


  it("加号菜单收纳输入框辅助动作", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    expect(view.getByRole("button", { name: "更多输入操作" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    expect(view.queryByRole("menuitem", { name: "添加附件" })).toBeNull();
    expect(view.queryByRole("menuitemcheckbox", { name: "计划模式" })).toBeNull();

    await openComposerActionMenu(view);

    expect(view.getByRole("menuitem", { name: "添加附件" })).toBeInTheDocument();
    expect(view.getByRole("menuitem", { name: "引用其他对话" })).toBeInTheDocument();
    expect(view.getByRole("menuitemcheckbox", { name: "计划模式" })).toHaveAttribute(
      "aria-checked",
      "false",
    );
    expect(view.getByRole("menuitemcheckbox", { name: "目标模式" })).toHaveAttribute(
      "aria-checked",
      "false",
    );
  });

  it("加号菜单会 Teleport 到 body，并在外部点击或 Escape 后关闭", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    await openComposerActionMenu(view);

    const menu = await view.findByRole("menu");
    expect(document.body.contains(menu)).toBe(true);
    expect(view.container.contains(menu)).toBe(false);

    await fireEvent.pointerDown(document.body);

    await waitFor(() => {
      expect(view.queryByRole("menu")).toBeNull();
    });

    await openComposerActionMenu(view);
    expect(await view.findByRole("menu")).toBeInTheDocument();

    await fireEvent.keyDown(document, { key: "Escape" });

    await waitFor(() => {
      expect(view.queryByRole("menu")).toBeNull();
    });
  });

  it("加号菜单会按触发位置设置展开 origin", async () => {
    const originalGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;
    const originalOffsetWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetWidth");
    const originalOffsetHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetHeight");
    Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", {
      configurable: true,
      value: function mockRect(this: HTMLElement) {
        if (this.classList.contains("chat-composer__action-menu")) {
          return {
            x: 100,
            y: 200,
            left: 100,
            top: 200,
            right: 128,
            bottom: 228,
            width: 28,
            height: 28,
            toJSON: () => ({}),
          } as DOMRect;
        }
        if (this.classList.contains("chat-composer__action-trigger")) {
          return {
            x: 100,
            y: 200,
            left: 100,
            top: 200,
            right: 128,
            bottom: 228,
            width: 28,
            height: 28,
            toJSON: () => ({}),
          } as DOMRect;
        }
        if (this.classList.contains("chat-composer__action-menu-popover")) {
          return {
            x: 100,
            y: 70,
            left: 100,
            top: 70,
            right: 290,
            bottom: 194,
            width: 190,
            height: 124,
            toJSON: () => ({}),
          } as DOMRect;
        }
        return originalGetBoundingClientRect.call(this);
      },
    });
    Object.defineProperty(HTMLElement.prototype, "offsetWidth", {
      configurable: true,
      get() {
        return this.classList.contains("chat-composer__action-menu-popover") ? 190 : 0;
      },
    });
    Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
      configurable: true,
      get() {
        return this.classList.contains("chat-composer__action-menu-popover") ? 124 : 0;
      },
    });

    try {
      const view = render(ChatComposer, {
        props: {
          state: baseState,
          attachments: [],
        },
      });

      await fireEvent.click(view.getByRole("button", { name: "更多输入操作" }), {
        clientX: 112,
        clientY: 214,
      });

      const menu = view.getByRole("menu");
      await waitFor(() => {
        expect(menu).toHaveStyle({
          "--sb-menu-origin-x": "12px",
          "--sb-menu-origin-y": "124px",
        });
      });
    } finally {
      Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", {
        configurable: true,
        value: originalGetBoundingClientRect,
      });
      if (originalOffsetWidth) {
        Object.defineProperty(HTMLElement.prototype, "offsetWidth", originalOffsetWidth);
      }
      if (originalOffsetHeight) {
        Object.defineProperty(HTMLElement.prototype, "offsetHeight", originalOffsetHeight);
      }
    }
  });

  it("加号菜单上方空间不足时会翻转到底部语义", async () => {
    const originalGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;
    const originalOffsetWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetWidth");
    const originalOffsetHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetHeight");
    const originalInnerHeight = window.innerHeight;
    Object.defineProperty(window, "innerHeight", {
      configurable: true,
      value: 360,
    });
    Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", {
      configurable: true,
      value: function mockRect(this: HTMLElement) {
        if (this.classList.contains("chat-composer__action-menu")) {
          return {
            x: 100,
            y: 24,
            left: 100,
            top: 24,
            right: 128,
            bottom: 52,
            width: 28,
            height: 28,
            toJSON: () => ({}),
          } as DOMRect;
        }
        if (this.classList.contains("chat-composer__action-trigger")) {
          return {
            x: 100,
            y: 24,
            left: 100,
            top: 24,
            right: 128,
            bottom: 52,
            width: 28,
            height: 28,
            toJSON: () => ({}),
          } as DOMRect;
        }
        if (this.classList.contains("chat-composer__action-menu-popover")) {
          return {
            x: 100,
            y: 58,
            left: 100,
            top: 58,
            right: 290,
            bottom: 182,
            width: 190,
            height: 124,
            toJSON: () => ({}),
          } as DOMRect;
        }
        return originalGetBoundingClientRect.call(this);
      },
    });
    Object.defineProperty(HTMLElement.prototype, "offsetWidth", {
      configurable: true,
      get() {
        return this.classList.contains("chat-composer__action-menu-popover") ? 190 : 0;
      },
    });
    Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
      configurable: true,
      get() {
        return this.classList.contains("chat-composer__action-menu-popover") ? 124 : 0;
      },
    });

    try {
      const view = render(ChatComposer, {
        props: {
          state: baseState,
          attachments: [],
        },
      });

      await fireEvent.click(view.getByRole("button", { name: "更多输入操作" }), {
        clientX: 112,
        clientY: 38,
      });

      const menu = await view.findByRole("menu");
      await waitFor(() => {
        expect(menu).toHaveClass("dd__menu--bottom");
        expect(menu).toHaveStyle({ top: "58px" });
      });
    } finally {
      Object.defineProperty(window, "innerHeight", {
        configurable: true,
        value: originalInnerHeight,
      });
      Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", {
        configurable: true,
        value: originalGetBoundingClientRect,
      });
      if (originalOffsetWidth) {
        Object.defineProperty(HTMLElement.prototype, "offsetWidth", originalOffsetWidth);
      }
      if (originalOffsetHeight) {
        Object.defineProperty(HTMLElement.prototype, "offsetHeight", originalOffsetHeight);
      }
    }
  });

  it("加号菜单可触发添加附件", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    await openComposerActionMenu(view);
    await fireEvent.click(view.getByRole("menuitem", { name: "添加附件" }));

    expect(view.emitted("pick-attachments")).toHaveLength(1);
    expect(view.queryByRole("menu")).toBeNull();
  });

  it("加号菜单会在输入框插入对话引用前缀", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    await openComposerActionMenu(view);
    await fireEvent.click(view.getByRole("menuitem", { name: "引用其他对话" }));

    expect(composerText(view.getByRole("textbox") as HTMLElement)).toContain("#");
    expect(view.queryByRole("menu")).toBeNull();
  });

  it("输入 # 后可搜索并插入对话引用", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    await setComposerText(view, "#Claude");
    await flushContextSearch();
    await fireEvent.click(view.getAllByRole("option")[0]);
    await fireEvent.click(view.getByRole("button", { name: "发送" }));

    expect(view.emitted("send")?.[0]?.[2]?.[0]).toEqual(expect.objectContaining({
      taskId: expect.any(String),
      title: expect.any(String),
      route: expect.any(String),
    }));
  });

  it("计划模式独立于执行权限切换", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    await openComposerActionMenu(view);
    const planButton = view.getByRole("menuitemcheckbox", { name: "计划模式" });
    expect(planButton).toHaveAttribute("aria-checked", "false");

    await fireEvent.click(planButton);

    expect(view.emitted("update:state")?.[0]?.[0]).toMatchObject({
      planMode: true,
      goalMode: false,
      permission: "full",
    });
  });

  it("目标模式独立于计划模式和执行权限切换", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    await openComposerActionMenu(view);
    const goalButton = view.getByRole("menuitemcheckbox", { name: "目标模式" });
    expect(goalButton).toHaveAttribute("aria-checked", "false");

    await fireEvent.click(goalButton);

    expect(view.emitted("update:state")?.[0]?.[0]).toMatchObject({
      planMode: false,
      goalMode: true,
      permission: "full",
    });
  });

  it("输入 /re 后可通过斜杠命令选择未提交改动审查", async () => {
    const view = render(ChatComposer, {
      props: {
        state: codexState,
        attachments: [],
        projectCwd,
      },
    });
    const input = view.getByRole("textbox") as HTMLElement;

    await setComposerText(view, "/re");
    await flushContextSearch();
    await fireEvent.click(view.getByRole("option", { name: /\/review/ }));

    expect(view.getByRole("option", { name: /未提交改动/ })).toBeInTheDocument();
    await fireEvent.click(view.getByRole("option", { name: /未提交改动/ }));

    expect(view.emitted("start-lilia-review")?.[0]).toEqual([
      "",
      [],
      [],
      { type: "uncommittedChanges" },
    ]);
    expect(view.emitted("execute-slash-command")).toBeUndefined();
    expect(composerText(input)).toBe("");
  });

  it("Codex review 斜杠命令会把输入框其他内容作为补充说明发出", async () => {
    const view = render(ChatComposer, {
      props: {
        state: codexState,
        attachments: [],
        projectCwd,
      },
    });
    await setComposerText(view, "重点看权限边界\n/re");
    await flushContextSearch();

    await fireEvent.click(view.getByRole("option", { name: /\/review/ }));
    await fireEvent.click(view.getByRole("option", { name: /未提交改动/ }));

    expect(view.emitted("start-lilia-review")?.[0]).toEqual([
      "重点看权限边界",
      [],
      [],
      { type: "uncommittedChanges" },
    ]);
  });

  it("Codex review 斜杠命令支持对比分支和指定提交目标", async () => {
    const promptSpy = vi.spyOn(window, "prompt");
    promptSpy.mockReturnValueOnce("main");
    promptSpy.mockReturnValueOnce("abc123");
    const view = render(ChatComposer, {
      props: {
        state: codexState,
        attachments: [],
        projectCwd,
      },
    });

    await setComposerText(view, "/re");
    await flushContextSearch();
    await fireEvent.click(view.getByRole("option", { name: /\/review/ }));
    await fireEvent.click(view.getByRole("option", { name: /对比分支/ }));
    await setComposerText(view, "/re");
    await flushContextSearch();
    await fireEvent.click(view.getByRole("option", { name: /\/review/ }));
    await fireEvent.click(view.getByRole("option", { name: /指定提交/ }));

    expect(view.emitted("start-lilia-review")?.[0]?.[3]).toEqual({
      type: "baseBranch",
      branch: "main",
    });
    expect(view.emitted("start-lilia-review")?.[1]?.[3]).toEqual({
      type: "commit",
      sha: "abc123",
    });
    promptSpy.mockRestore();
  });

  it("输入 /fi 后可通过斜杠命令发起修复建议", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        projectCwd,
      },
    });

    await setComposerText(view, "/fi");
    await flushContextSearch();
    await fireEvent.click(view.getByRole("option", { name: /\/fix/ }));
    await fireEvent.click(view.getByRole("option", { name: /未提交改动/ }));

    expect(view.emitted("start-lilia-fix-suggestion")?.[0]).toEqual([
      "",
      [],
      [],
      { type: "uncommittedChanges" },
    ]);

    await view.rerender({
      state: codexState,
      attachments: [],
      projectCwd,
    });
    await setComposerText(view, "/fi");
    await flushContextSearch();
    await fireEvent.click(view.getByRole("option", { name: /\/fix/ }));
    await fireEvent.click(view.getByRole("option", { name: /未提交改动/ }));
    expect(view.emitted("start-lilia-fix-suggestion")?.[1]).toEqual([
      "",
      [],
      [],
      { type: "uncommittedChanges" },
    ]);
  });

  it("Codex 修复建议会把输入框内容作为补充说明发出", async () => {
    const view = render(ChatComposer, {
      props: {
        state: codexState,
        attachments: [],
        projectCwd,
      },
    });
    await setComposerText(view, "优先给最小修复\n/fi");
    await flushContextSearch();

    await fireEvent.click(view.getByRole("option", { name: /\/fix/ }));
    await fireEvent.click(view.getByRole("option", { name: /未提交改动/ }));

    expect(view.emitted("start-lilia-fix-suggestion")?.[0]).toEqual([
      "优先给最小修复",
      [],
      [],
      { type: "uncommittedChanges" },
    ]);
  });

  it("Codex 修复建议支持对比分支和指定提交目标", async () => {
    const promptSpy = vi.spyOn(window, "prompt");
    promptSpy.mockReturnValueOnce("main");
    promptSpy.mockReturnValueOnce("abc123");
    const view = render(ChatComposer, {
      props: {
        state: codexState,
        attachments: [],
        projectCwd,
      },
    });

    await setComposerText(view, "/fi");
    await flushContextSearch();
    await fireEvent.click(view.getByRole("option", { name: /\/fix/ }));
    await fireEvent.click(view.getByRole("option", { name: /对比分支/ }));
    await setComposerText(view, "/fi");
    await flushContextSearch();
    await fireEvent.click(view.getByRole("option", { name: /\/fix/ }));
    await fireEvent.click(view.getByRole("option", { name: /指定提交/ }));

    expect(view.emitted("start-lilia-fix-suggestion")?.[0]?.[3]).toEqual({
      type: "baseBranch",
      branch: "main",
    });
    expect(view.emitted("start-lilia-fix-suggestion")?.[1]?.[3]).toEqual({
      type: "commit",
      sha: "abc123",
    });
    promptSpy.mockRestore();
  });

  it("工具栏不再显示代码审查和修复建议入口", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    expect(view.queryByRole("button", { name: "代码审查" })).toBeNull();
    expect(view.queryByRole("button", { name: "修复建议" })).toBeNull();
  });

  it("阻塞 pending 交互时不显示斜杠 workflow 入口", async () => {
    const view = render(ChatComposer, {
      props: {
        state: codexState,
        attachments: [],
        pendingAsk: pendingAsk(singleAskWithOtherSpec),
        projectCwd,
      },
    });

    expect(view.queryByRole("button", { name: "代码审查" })).toBeNull();
    expect(view.queryByRole("listbox", { name: "斜杠命令" })).toBeNull();
    expect(view.emitted("start-lilia-review")).toBeUndefined();
    expect(view.emitted("start-lilia-fix-suggestion")).toBeUndefined();
  });

  it("Claude 和 Codex 后端可从工具栏发起上下文压缩", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    expect(view.queryByTitle("压缩上下文")).toBeNull();
    const compactButton = view.getByRole("button", { name: /压缩上下文/ });
    expect(compactButton).not.toBeDisabled();
    expect(compactButton).toHaveClass("chat-composer__context-action");
    await fireEvent.click(compactButton);

    expect(view.emitted("start-lilia-compact")?.length).toBe(1);

    await view.rerender({
      state: codexState,
      attachments: [],
    });
    await fireEvent.click(view.getByRole("button", { name: /压缩上下文/ }));
    expect(view.emitted("start-lilia-compact")?.length).toBe(2);
  });

  it("上下文圆环展示已用比例和具体占用信息", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        contextUsage: {
          taskId: "task-1",
          backend: "claude",
          usedTokens: 7168,
          limitTokens: 8192,
          usedPercent: 87.5,
          source: "runtime",
          updatedAt: Date.UTC(2026, 5, 16, 8, 30),
          unavailableReason: null,
        },
      },
    });

    const compactButton = view.getByRole("button", { name: /已用 7,168 tokens/ });
    expect(compactButton).toHaveClass("chat-context-ring--warn");
    expect(compactButton).toHaveStyle({ "--quota-progress": "87.5" });
    const contextCard = view.getByRole("tooltip");
    expect(contextCard).toHaveTextContent("已用");
    expect(contextCard).toHaveTextContent("占用");
    expect(contextCard).toHaveTextContent("87.5%");
  });

  it("上下文圆环无比例时显示空态但仍可触发压缩", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        contextUsage: {
          taskId: "task-1",
          backend: "claude",
          usedTokens: 2048,
          limitTokens: null,
          usedPercent: null,
          source: "runtime",
          updatedAt: Date.UTC(2026, 5, 16, 8, 30),
          unavailableReason: "provider 未返回上下文上限",
        },
      },
    });

    const compactButton = view.getByRole("button", { name: /占用比例未知/ });
    expect(compactButton).toHaveClass("chat-context-ring--empty");
    expect(compactButton).toHaveStyle({ "--quota-progress": "0" });
    expect(view.getByRole("tooltip")).toHaveTextContent("provider 未返回上下文上限");
    await fireEvent.click(compactButton);
    expect(view.emitted("start-lilia-compact")?.length).toBe(1);
  });

  it("Claude 和 Codex 后端不再从工具栏分叉当前会话", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    expect(view.queryByRole("button", { name: "分叉当前会话" })).toBeNull();

    await view.rerender({
      state: codexState,
      attachments: [],
    });

    expect(view.queryByRole("button", { name: "分叉当前会话" })).toBeNull();
    expect(view.emitted("start-session-fork")).toBeUndefined();
  });

  it("工具栏不再暴露 IAB 入口", async () => {
    const view = render(ChatComposer, {
      props: {
        state: codexState,
        attachments: [],
      },
    });

    expect(view.queryByRole("button", { name: "打开 Lilia IAB" })).toBeNull();
    expect(view.queryByRole("button", { name: "回送 IAB 截图" })).toBeNull();
  });

  it("运行中也不在工具栏暴露 IAB 入口", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    expect(view.queryByRole("button", { name: "打开 Lilia IAB" })).toBeNull();
    expect(view.queryByRole("button", { name: "回送 IAB 截图" })).toBeNull();

    await view.rerender({
      state: codexState,
      attachments: [],
      sending: true,
    });

    expect(view.queryByRole("button", { name: "打开 Lilia IAB" })).toBeNull();
    expect(view.queryByRole("button", { name: "回送 IAB 截图" })).toBeNull();
  });

  it("compact 入口在运行中或禁用状态时禁用", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        sending: true,
      },
    });

    expect(view.getByRole("button", { name: /压缩上下文/ })).toBeDisabled();

    await view.rerender({
      state: baseState,
      attachments: [],
      sending: false,
      compactDisabled: true,
    });
    expect(view.getByRole("button", { name: /压缩上下文/ })).toBeDisabled();
  });

  it("Codex 后端不再从工具栏清理后台终端", async () => {
    const view = render(ChatComposer, {
      props: {
        state: codexState,
        attachments: [],
      },
    });

    expect(view.queryByRole("button", { name: "清理 Codex 后台终端" })).toBeNull();
  });

  it("Codex 后端不再显示原生接口、Memory、Goal、Fork 和配置诊断入口", async () => {
    const view = render(ChatComposer, {
      props: {
        state: codexState,
        attachments: [],
      },
    });

    expect(view.queryByRole("button", { name: "Codex 原生接口" })).toBeNull();
    expect(view.queryByRole("button", { name: "Codex 高级字段" })).toBeNull();
    expect(view.queryByRole("menuitem", { name: "启用 Memory" })).toBeNull();
    expect(view.queryByRole("menuitem", { name: "关闭 Memory" })).toBeNull();
    expect(view.queryByRole("menuitem", { name: /重置 Memory/ })).toBeNull();
    expect(view.queryByRole("menuitem", { name: /设置 Goal/ })).toBeNull();
    expect(view.queryByRole("menuitem", { name: "Fork 当前 Thread" })).toBeNull();
    expect(view.queryByRole("menuitem", { name: "读取配置诊断" })).toBeNull();
  });

  it("原生接口入口在非 Codex、运行中和阻塞 pending 时都不显示", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    expect(view.queryByRole("button", { name: "Codex 原生接口" })).toBeNull();

    await view.rerender({
      state: codexState,
      attachments: [],
      sending: true,
    });
    expect(view.queryByRole("button", { name: "Codex 原生接口" })).toBeNull();

    await view.rerender({
      state: codexState,
      attachments: [],
      sending: false,
      compactDisabled: true,
    });
    expect(view.queryByRole("button", { name: "Codex 原生接口" })).toBeNull();
  });

  it("后台终端清理入口不再显示在 composer 工具栏", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    expect(view.queryByRole("button", { name: "清理 Codex 后台终端" })).toBeNull();

    await view.rerender({
      state: codexState,
      attachments: [],
      sending: true,
    });
    expect(view.queryByRole("button", { name: "清理 Codex 后台终端" })).toBeNull();

    await view.rerender({
      state: codexState,
      attachments: [],
      sending: false,
      compactDisabled: true,
    });
    expect(view.queryByRole("button", { name: "清理 Codex 后台终端" })).toBeNull();
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

  it("状态切到工具授权后短暂屏蔽关键动作", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
      },
    });

    await view.rerender({
      state: baseState,
      attachments: [],
      toolConsent,
    });
    await fireEvent.update(view.getByRole("textbox"), "先不要写这个文件");

    expect(view.getByRole("button", { name: "同意" })).toBeDisabled();
    expect(view.getByRole("button", { name: "修改" })).toBeDisabled();

    await vi.advanceTimersByTimeAsync(320);
    expect(view.getByRole("button", { name: "同意" })).not.toBeDisabled();
    expect(view.getByRole("button", { name: "修改" })).not.toBeDisabled();
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

  it("pending Codex command approval 同意时返回用户修改后的 updatedInput", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        attachments: [],
        toolConsent: codexCommandToolConsent,
      },
    });

    await fireEvent.click(view.getByRole("button", { name: "编辑完整命令" }));
    await fireEvent.update(view.getByRole("textbox", { name: "编辑命令" }), "yarn test --runInBand");
    await fireEvent.click(view.getByRole("button", { name: "同意" }));

    expect(view.emitted("resolve-tool-consent")?.[0]).toEqual([
      "allow",
      undefined,
      { command: "yarn test --runInBand" },
    ]);
  });
});
