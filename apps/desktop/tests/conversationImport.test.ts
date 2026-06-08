import { fireEvent, render, waitFor, within } from "@testing-library/vue";
import { describe, expect, it, vi } from "vitest";
import type { AgentTimelineEvent, ClaudeSessionSummary, CodexThreadSummary } from "@lilia/contracts";
import ConversationImport from "../src/pages/ConversationImport.vue";
import {
  attachClaudeSession,
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
        eventCount: 2,
        messages: [],
        hasFullPreview: true,
        events: [],
      };
    });

    const view = render(ConversationImport);

    await waitFor(() => {
      expect(view.container.querySelector(".agent-timeline")).toBeInTheDocument();
    });

    const sidebar = view.container.querySelector(".conversation-import__sidebar");
    expect(sidebar).toBeInstanceOf(HTMLElement);
    expect(within(sidebar as HTMLElement).getByRole("tab", { name: "Codex" }))
      .toHaveClass("is-active");
    expect(within(sidebar as HTMLElement).getByRole("button", { name: /优化导入对话界面/ }))
      .toBeInTheDocument();
    expect(view.getByRole("button", { name: "导入到收集箱" })).toBeEnabled();
    expect(previewCodexThread).toHaveBeenCalledWith({ threadId: "thread-1", detail: "full" });
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
