import { fireEvent, render, waitFor, within } from "@testing-library/vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  AgentTimelineEvent,
  ClaudeSessionSummary,
  CodexThreadRuntimeState,
  CodexThreadSummary,
} from "@lilia/contracts";
import ConversationImport from "../src/pages/ConversationImport.vue";
import {
  attachClaudeSession,
  cleanCodexThreadBackgroundTerminals,
  listCodexThreadRuntimeStates,
  previewClaudeSession,
  previewCodexThread,
  searchClaudeSessions,
  searchCodexThreads,
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
  attachClaudeSession: vi.fn(async () => ({
    taskId: "task-claude-imported",
    projectId: null,
    sessionId: "claude-session-1",
    task: null,
    eventCount: 0,
  })),
  attachCodexThread: vi.fn(async () => ({
    taskId: "task-imported",
    projectId: null,
    threadId: "thread-1",
    task: null,
    eventCount: 0,
  })),
  cleanCodexThreadBackgroundTerminals: vi.fn(async () => undefined),
  listCodexThreadRuntimeStates: vi.fn(async () => []),
  previewClaudeSession: vi.fn(),
  previewCodexThread: vi.fn(),
  searchClaudeSessions: vi.fn(),
  searchCodexThreads: vi.fn(),
}));

function threadSummary(patch: Partial<CodexThreadSummary> = {}): CodexThreadSummary {
  return {
    id: "thread-1",
    title: "优化导入对话界面",
    status: "completed",
    model: "gpt-5.4",
    sourceKind: "codex",
    createdAt: 1735689600000,
    updatedAt: 1735693200000,
    archived: false,
    preview: "这段对话内容不应该出现在左侧列表",
    ...patch,
  };
}

function claudeSessionSummary(patch: Partial<ClaudeSessionSummary> = {}): ClaudeSessionSummary {
  return {
    id: "claude-session-1",
    title: "整理 Claude 历史导入",
    status: null,
    model: "claude-sonnet-4-5",
    sourceKind: "claude",
    createdAt: 1735689600000,
    updatedAt: 1735696800000,
    archived: false,
    preview: "Claude 历史摘要不出现在左侧行内",
    cwd: "D:\\PROJECT\\workspace\\Lilia",
    project: "d--PROJECT-workspace-Lilia",
    ...patch,
  };
}

function runtimeState(patch: Partial<CodexThreadRuntimeState> = {}): CodexThreadRuntimeState {
  return {
    threadId: "thread-1",
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

describe("ConversationImport", () => {
  beforeEach(() => {
    routerMock.push.mockClear();
    vi.mocked(attachClaudeSession).mockClear();
    vi.mocked(cleanCodexThreadBackgroundTerminals).mockClear();
    vi.mocked(listCodexThreadRuntimeStates).mockResolvedValue([]);
    vi.mocked(previewClaudeSession).mockReset();
    vi.mocked(previewCodexThread).mockReset();
    vi.mocked(searchClaudeSessions).mockReset();
    vi.mocked(searchCodexThreads).mockReset();
  });

  it("renders Codex history preview by default", async () => {
    const thread = threadSummary();
    vi.mocked(searchCodexThreads).mockResolvedValue({
      threads: [thread],
      nextCursor: null,
    });
    vi.mocked(previewCodexThread).mockImplementation(async (input) => {
      if (input.detail === "full") {
        return {
          thread,
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
        thread,
        eventCount: 99,
        messages: [],
        hasFullPreview: true,
        events: [],
      };
    });

    const view = render(ConversationImport);

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
    expect(previewCodexThread).toHaveBeenCalledWith({ threadId: "thread-1", detail: "full" });
  });

  it("先显示 Lilia 管理会话，再合并后台 Codex 历史", async () => {
    let resolveSearch: (value: { threads: CodexThreadSummary[]; nextCursor: string | null }) => void;
    const searchPromise = new Promise<{ threads: CodexThreadSummary[]; nextCursor: string | null }>((resolve) => {
      resolveSearch = resolve;
    });
    vi.mocked(listCodexThreadRuntimeStates).mockResolvedValue([
      runtimeState({
        threadId: "thread-lilia",
        taskTitle: "Lilia 本地 Codex 对话",
      }),
    ]);
    vi.mocked(searchCodexThreads).mockReturnValue(searchPromise);
    vi.mocked(previewCodexThread).mockResolvedValue({
      thread: threadSummary({ id: "thread-lilia", title: "Lilia 本地 Codex 对话" }),
      eventCount: 0,
      messages: [],
      hasFullPreview: false,
      events: [],
    });

    const view = render(ConversationImport);
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
      threads: [
        threadSummary({
          id: "thread-lilia",
          title: "Codex 历史补全标题",
          model: "gpt-5.5",
          preview: "app-server 返回的预览摘要。",
        }),
        threadSummary({
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
    let resolveSearch: (value: { threads: CodexThreadSummary[]; nextCursor: string | null }) => void;
    const searchPromise = new Promise<{ threads: CodexThreadSummary[]; nextCursor: string | null }>((resolve) => {
      resolveSearch = resolve;
    });
    vi.mocked(listCodexThreadRuntimeStates).mockResolvedValue([
      runtimeState({
        threadId: "thread-local-search",
        taskTitle: "只在 Lilia 里的任务",
      }),
      runtimeState({
        threadId: "thread-other",
        taskTitle: "另一个任务",
      }),
    ]);
    vi.mocked(searchCodexThreads).mockReturnValue(searchPromise);
    vi.mocked(previewCodexThread).mockResolvedValue({
      thread: threadSummary({ id: "thread-local-search", title: "只在 Lilia 里的任务" }),
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

    resolveSearch!({ threads: [], nextCursor: null });
  });

  it("在 Codex 导入列表显示 Lilia 管理状态并清理运行中后台终端", async () => {
    const thread = threadSummary({
      id: "thread-running",
      title: "整理 Codex 会话管理",
      preview: "讨论设置页中的会话维护入口。",
    });
    vi.mocked(searchCodexThreads).mockResolvedValue({
      threads: [thread],
      nextCursor: null,
    });
    vi.mocked(listCodexThreadRuntimeStates).mockResolvedValue([
      runtimeState({
        threadId: "thread-running",
        taskId: "task-running",
        taskTitle: "打通 tsconfig paths 搜索",
        running: true,
        pending: true,
      }),
    ]);
    vi.mocked(previewCodexThread).mockResolvedValue({
      thread,
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
      expect(cleanCodexThreadBackgroundTerminals).toHaveBeenCalledWith("thread-running");
      expect(within(sidebar as HTMLElement).getByText("后台终端已清理")).toBeInTheDocument();
    });
  });

  it("切换到 Claude 后搜索、预览并导入 Claude session", async () => {
    const thread = threadSummary();
    const session = claudeSessionSummary();
    vi.mocked(searchCodexThreads).mockResolvedValue({
      threads: [thread],
      nextCursor: null,
    });
    vi.mocked(previewCodexThread).mockResolvedValue({
      thread,
      eventCount: 0,
      messages: [],
      hasFullPreview: false,
      events: [],
    });
    vi.mocked(searchClaudeSessions).mockResolvedValue({
      sessions: [session],
      nextCursor: null,
    });
    vi.mocked(previewClaudeSession).mockImplementation(async (input) => {
      if (input.detail === "full") {
        return {
          session,
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
        session,
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
    expect(previewClaudeSession).toHaveBeenCalledWith({
      sessionId: "claude-session-1",
      detail: "full",
    });

    await fireEvent.click(view.getByRole("button", { name: "导入到收集箱" }));
    await waitFor(() => {
      expect(attachClaudeSession).toHaveBeenCalledWith({
        mode: "new",
        sessionId: "claude-session-1",
        taskId: null,
        projectId: null,
        session,
      });
    });
    expect(routerMock.push).toHaveBeenCalledWith("/chats/task-claude-imported");
  });
});
