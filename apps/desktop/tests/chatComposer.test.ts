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

    expect(view.emitted("send")?.[0]).toEqual(["补充上下文", []]);
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
