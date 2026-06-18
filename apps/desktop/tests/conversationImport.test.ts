import { fireEvent, render, waitFor, within } from "@testing-library/vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  AgentTimelineEvent,
  HistoryImportItem,
  HistoryImportRuntimeState,
} from "@lilia/contracts";
import ConversationImport from "../src/pages/ConversationImport.vue";
import {
  attachHistoryImport,
  cleanHistoryImportBackgroundTerminals,
  listHistoryImportRuntimeStates,
  previewHistoryImport,
  searchHistoryImports,
} from "../src/services/chat";

const routerMock = vi.hoisted(() => ({
  push: vi.fn(),
}));

vi.mock("vue-router", () => ({
  useRoute: () => ({ query: {} }),
  useRouter: () => ({ push: routerMock.push }),
}));

vi.mock("../src/services/tasksStore", () => ({
  ensureOrphansLoaded: vi.fn(async () => undefined),
  ensureProjectTasksLoaded: vi.fn(async () => undefined),
}));

vi.mock("../src/services/chat", () => ({
  attachHistoryImport: vi.fn(async (input: { provider: "codex" | "claude"; itemId: string }) => ({
    taskId: input.provider === "claude" ? "task-claude-imported" : "task-imported",
    projectId: null,
    itemId: input.itemId,
    task: null,
    eventCount: 0,
  })),
  cleanHistoryImportBackgroundTerminals: vi.fn(async () => undefined),
  listHistoryImportRuntimeStates: vi.fn(async () => []),
  previewHistoryImport: vi.fn(),
  searchHistoryImports: vi.fn(),
}));

function historyImportItem(patch: Partial<HistoryImportItem> = {}): HistoryImportItem {
  const provider = patch.provider ?? "codex";
  return {
    id: provider === "claude" ? "claude-session-1" : "thread-1",
    provider,
    title: provider === "claude" ? "整理 Claude 历史导入" : "优化导入对话界面",
    status: provider === "claude" ? null : "completed",
    model: provider === "claude" ? "claude-sonnet-4-5" : "gpt-5.4",
    sourceKind: provider,
    createdAt: 1735689600000,
    updatedAt: provider === "claude" ? 1735696800000 : 1735693200000,
    archived: false,
    preview: provider === "claude"
      ? "Claude 历史摘要不出现在左侧行内"
      : "这段对话内容不应该出现在左侧列表",
    cwd: provider === "claude" ? "D:\\PROJECT\\workspace\\Lilia" : null,
    project: provider === "claude" ? "d--PROJECT-workspace-Lilia" : null,
    ...patch,
  };
}

function historyImportRuntimeState(patch: Partial<HistoryImportRuntimeState> = {}): HistoryImportRuntimeState {
  return {
    itemId: "thread-1",
    taskId: "task-1",
    taskTitle: "优化导入对话界面",
    projectId: null,
    running: false,
    queued: false,
    pending: false,
    queuedCount: 0,
    ...patch,
  };
}

function timelineEvent(
  patch: Partial<AgentTimelineEvent> & Pick<AgentTimelineEvent, "id" | "kind" | "payload">,
): AgentTimelineEvent {
  return {
    id: patch.id,
    taskId: "preview",
    turnId: null,
    backend: "codex",
    kind: patch.kind,
    status: patch.status ?? "success",
    title: patch.title ?? patch.kind,
    summary: patch.summary ?? "",
    payload: patch.payload,
    createdAt: patch.createdAt ?? 1735693200000,
    updatedAt: patch.updatedAt ?? patch.createdAt ?? 1735693200000,
    turnSeq: patch.turnSeq ?? 1,
    intraTurnOrder: patch.intraTurnOrder ?? 1,
  };
}

async function flushAsyncPreviewComponents() {
  if (typeof vi.dynamicImportSettled === "function") {
    await vi.dynamicImportSettled();
  }
  await Promise.resolve();
  await Promise.resolve();
}

describe("ConversationImport", () => {
  beforeEach(() => {
    routerMock.push.mockClear();
    vi.mocked(attachHistoryImport).mockClear();
    vi.mocked(cleanHistoryImportBackgroundTerminals).mockClear();
    vi.mocked(listHistoryImportRuntimeStates).mockResolvedValue([]);
    vi.mocked(previewHistoryImport).mockReset();
    vi.mocked(searchHistoryImports).mockReset();
  });

  it("renders Codex history preview by default", async () => {
    const thread = historyImportItem();
    vi.mocked(searchHistoryImports).mockResolvedValue({
      items: [thread],
      nextCursor: null,
    });
    vi.mocked(previewHistoryImport).mockImplementation(async (input) => {
      if (input.detail === "full") {
        return {
          item: thread,
          eventCount: 2,
          messages: [],
          hasFullPreview: true,
          events: [
            timelineEvent({
              id: "user-1",
              kind: "message",
              payload: { role: "user", content: "请优化导入界面" },
            }),
            timelineEvent({
              id: "final-1",
              kind: "message",
              payload: { role: "assistant", content: "已完成 UI 调整" },
              intraTurnOrder: 2,
            }),
          ],
        };
      }
      return {
        item: thread,
        eventCount: 99,
        messages: [],
        hasFullPreview: true,
        events: [],
      };
    });

    const view = render(ConversationImport);
    await flushAsyncPreviewComponents();

    await waitFor(() => {
      expect(view.container.querySelector(".agent-timeline")).toBeInTheDocument();
    });
    expect(view.getByText(/2 条事件/)).toBeInTheDocument();
    expect(view.queryByText(/99 条事件/)).not.toBeInTheDocument();

    const sidebar = view.container.querySelector(".conversation-import__sidebar");
    expect(sidebar).toBeInstanceOf(HTMLElement);
    expect(within(sidebar as HTMLElement).getByRole("tab", { name: "Codex" }))
      .toHaveClass("is-active");
    expect(within(sidebar as HTMLElement).getByRole("button", { name: /优化导入对话界面/ }))
      .toBeInTheDocument();
    expect(view.getByRole("button", { name: "导入到收集箱" })).toBeEnabled();
    expect(previewHistoryImport).toHaveBeenCalledWith({
      provider: "codex",
      itemId: "thread-1",
      detail: "full",
    });
  });

  it("先显示 Lilia 管理会话，再合并后台 Codex 历史", async () => {
    let resolveSearch: (value: { items: HistoryImportItem[]; nextCursor: string | null }) => void;
    const searchPromise = new Promise<{ items: HistoryImportItem[]; nextCursor: string | null }>((resolve) => {
      resolveSearch = resolve;
    });
    vi.mocked(listHistoryImportRuntimeStates).mockResolvedValue([
      historyImportRuntimeState({
        itemId: "thread-lilia",
        taskTitle: "Lilia 本地 Codex 对话",
      }),
    ]);
    vi.mocked(searchHistoryImports).mockReturnValue(searchPromise);
    vi.mocked(previewHistoryImport).mockResolvedValue({
      item: historyImportItem({ id: "thread-lilia", title: "Lilia 本地 Codex 对话" }),
      eventCount: 0,
      messages: [],
      hasFullPreview: false,
      events: [],
    });

    const view = render(ConversationImport);
    await flushAsyncPreviewComponents();
    const sidebar = view.container.querySelector(".conversation-import__sidebar");
    expect(sidebar).toBeInstanceOf(HTMLElement);

    await waitFor(() => {
      expect(within(sidebar as HTMLElement).getByRole("button", {
        name: /Lilia 本地 Codex 对话/,
      })).toBeInTheDocument();
    });
    expect(within(sidebar as HTMLElement).getByText("LiliaCode")).toBeInTheDocument();
    expect(within(sidebar as HTMLElement).queryByText("Lilia 管理")).not.toBeInTheDocument();
    expect(within(sidebar as HTMLElement).queryByText("远端 Codex 历史")).not.toBeInTheDocument();

    resolveSearch!({
      items: [
        historyImportItem({
          id: "thread-lilia",
          title: "Codex 历史补全标题",
          model: "gpt-5.5",
          preview: "app-server 返回的预览摘要。",
        }),
        historyImportItem({
          id: "thread-remote",
          title: "远端 Codex 历史",
          preview: "另一个 app-server thread。",
        }),
      ],
      nextCursor: null,
    });

    await waitFor(() => {
      expect(within(sidebar as HTMLElement).getByRole("button", {
        name: /Codex 历史补全标题/,
      })).toBeInTheDocument();
      expect(within(sidebar as HTMLElement).getByRole("button", {
        name: /远端 Codex 历史/,
      })).toBeInTheDocument();
    });
    expect(within(sidebar as HTMLElement).getAllByText("LiliaCode")).toHaveLength(1);
    expect(within(sidebar as HTMLElement).queryByText("gpt-5.5")).not.toBeInTheDocument();
    expect(within(sidebar as HTMLElement).queryByText("app-server 返回的预览摘要。"))
      .not.toBeInTheDocument();
  });

  it("搜索词可直接命中 Lilia 本地任务标题", async () => {
    let resolveSearch: (value: { items: HistoryImportItem[]; nextCursor: string | null }) => void;
    const searchPromise = new Promise<{ items: HistoryImportItem[]; nextCursor: string | null }>((resolve) => {
      resolveSearch = resolve;
    });
    vi.mocked(listHistoryImportRuntimeStates).mockResolvedValue([
      historyImportRuntimeState({
        itemId: "thread-local-search",
        taskTitle: "只在 Lilia 里的任务",
      }),
      historyImportRuntimeState({
        itemId: "thread-other",
        taskTitle: "另一个任务",
      }),
    ]);
    vi.mocked(searchHistoryImports).mockReturnValue(searchPromise);
    vi.mocked(previewHistoryImport).mockResolvedValue({
      item: historyImportItem({ id: "thread-local-search", title: "只在 Lilia 里的任务" }),
      eventCount: 0,
      messages: [],
      hasFullPreview: false,
      events: [],
    });

    const view = render(ConversationImport);
    const sidebar = view.container.querySelector(".conversation-import__sidebar");
    expect(sidebar).toBeInstanceOf(HTMLElement);
    const search = within(sidebar as HTMLElement).getByRole("searchbox", {
      name: "搜索 Codex thread",
    });
    await fireEvent.update(search, "只在 Lilia");
    await new Promise((resolve) => window.setTimeout(resolve, 280));

    await waitFor(() => {
      expect(within(sidebar as HTMLElement).getByRole("button", {
        name: /只在 Lilia 里的任务/,
      })).toBeInTheDocument();
    });
    expect(within(sidebar as HTMLElement).queryByRole("button", {
      name: /另一个任务/,
    })).not.toBeInTheDocument();

    resolveSearch!({ items: [], nextCursor: null });
  });

  it("在 Codex 导入列表显示 Lilia 管理状态并清理运行中后台终端", async () => {
    const thread = historyImportItem({
      id: "thread-running",
      title: "整理 Codex 会话管理",
      preview: "讨论设置页中的会话维护入口。",
    });
    vi.mocked(searchHistoryImports).mockResolvedValue({
      items: [thread],
      nextCursor: null,
    });
    vi.mocked(listHistoryImportRuntimeStates).mockResolvedValue([
      historyImportRuntimeState({
        itemId: "thread-running",
        taskId: "task-running",
        taskTitle: "打通 tsconfig paths 搜索",
        running: true,
        pending: true,
      }),
    ]);
    vi.mocked(previewHistoryImport).mockResolvedValue({
      item: thread,
      eventCount: 0,
      messages: [],
      hasFullPreview: false,
      events: [],
    });

    const view = render(ConversationImport);
    const sidebar = view.container.querySelector(".conversation-import__sidebar");
    expect(sidebar).toBeInstanceOf(HTMLElement);

    await waitFor(() => {
      expect(within(sidebar as HTMLElement).getByText("运行中")).toBeInTheDocument();
    });
    expect(within(sidebar as HTMLElement).getByText("LiliaCode")).toBeInTheDocument();
    expect(within(sidebar as HTMLElement).queryByText("Lilia: 打通 tsconfig paths 搜索"))
      .not.toBeInTheDocument();
    expect(within(sidebar as HTMLElement).queryByText("讨论设置页中的会话维护入口。"))
      .not.toBeInTheDocument();
    expect(within(sidebar as HTMLElement).queryByRole("button", { name: "归档" }))
      .not.toBeInTheDocument();
    expect(within(sidebar as HTMLElement).queryByRole("button", { name: "取消归档" }))
      .not.toBeInTheDocument();

    await fireEvent.click(within(sidebar as HTMLElement).getByRole("button", {
      name: "清理后台终端",
    }));

    await waitFor(() => {
      expect(cleanHistoryImportBackgroundTerminals).toHaveBeenCalledWith("thread-running");
      expect(within(sidebar as HTMLElement).getByText("后台终端已清理")).toBeInTheDocument();
    });
  });

  it("切换到 Claude 后搜索、预览并导入 Claude session", async () => {
    const thread = historyImportItem();
    const session = historyImportItem({ provider: "claude" });
    vi.mocked(searchHistoryImports).mockResolvedValueOnce({
      items: [thread],
      nextCursor: null,
    });
    vi.mocked(previewHistoryImport).mockResolvedValue({
      item: thread,
      eventCount: 0,
      messages: [],
      hasFullPreview: false,
      events: [],
    });
    vi.mocked(searchHistoryImports).mockResolvedValueOnce({
      items: [session],
      nextCursor: null,
    });
    vi.mocked(previewHistoryImport).mockImplementation(async (input) => {
      if (input.detail === "full") {
        return {
          item: session,
          eventCount: 2,
          messages: [],
          hasFullPreview: true,
          events: [
            timelineEvent({
              id: "claude-user-1",
              backend: "claude",
              kind: "message",
              payload: { role: "user", content: "补齐 Claude 导入" },
            }),
            timelineEvent({
              id: "claude-assistant-1",
              backend: "claude",
              kind: "message",
              payload: { role: "assistant", content: "已完成 Claude 导入" },
              intraTurnOrder: 2,
            }),
          ],
        };
      }
      return {
        item: session,
        eventCount: 2,
        messages: [],
        hasFullPreview: true,
        events: [],
      };
    });

    const view = render(ConversationImport);
    const sidebar = view.container.querySelector(".conversation-import__sidebar");
    expect(sidebar).toBeInstanceOf(HTMLElement);

    await fireEvent.click(within(sidebar as HTMLElement).getByRole("tab", { name: "Claude" }));
    await flushAsyncPreviewComponents();

    await waitFor(() => {
      expect(within(sidebar as HTMLElement).getByRole("button", {
        name: /整理 Claude 历史导入/,
      })).toBeInTheDocument();
    });
    expect(within(sidebar as HTMLElement).getByRole("tab", { name: "Claude" }))
      .toHaveClass("is-active");

    await waitFor(() => {
      expect(view.container.querySelector(".agent-timeline")).toBeInTheDocument();
    });
    expect(previewHistoryImport).toHaveBeenCalledWith({
      provider: "claude",
      itemId: "claude-session-1",
      detail: "full",
    });

    await fireEvent.click(view.getByRole("button", { name: "导入到收集箱" }));
    await waitFor(() => {
      expect(attachHistoryImport).toHaveBeenCalledWith({
        provider: "claude",
        mode: "new",
        itemId: "claude-session-1",
        taskId: null,
        projectId: null,
        item: session,
      });
    });
    expect(routerMock.push).toHaveBeenCalledWith("/chats/task-claude-imported");
  });
});
