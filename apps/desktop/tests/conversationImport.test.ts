import { fireEvent, render, waitFor, within } from "@testing-library/vue";
import { describe, expect, it, vi } from "vitest";
import type { AgentTimelineEvent, CodexThreadSummary } from "@lilia/contracts";
import ConversationImport from "../src/pages/ConversationImport.vue";
import {
  previewCodexThread,
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
  attachCodexThread: vi.fn(async () => ({
    taskId: "task-imported",
    projectId: null,
    threadId: "thread-1",
    task: null,
    eventCount: 0,
  })),
  previewCodexThread: vi.fn(),
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

function mockScrollablePreview(view: { container: HTMLElement }) {
  const scroller = view.container.querySelector(".conversation-import__timeline-scroller");
  expect(scroller).toBeInstanceOf(HTMLElement);
  Object.defineProperty(scroller, "clientHeight", {
    configurable: true,
    value: 160,
  });
  Object.defineProperty(scroller, "scrollHeight", {
    configurable: true,
    value: 800,
  });
  vi.spyOn(scroller as HTMLElement, "getBoundingClientRect").mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 520,
    bottom: 160,
    width: 520,
    height: 160,
    toJSON: () => ({}),
  });
  return scroller as HTMLElement;
}

describe("ConversationImport", () => {
  it("按导入页新布局渲染来源、列表和时间线详情", async () => {
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

    expect(view.queryByRole("heading", { name: "导入对话" })).not.toBeInTheDocument();
    expect(view.queryByText("从已有 Claude / Codex 历史中选择一个对话，导入后继续处理。"))
      .not.toBeInTheDocument();

    const sidebar = view.container.querySelector(".conversation-import__sidebar");
    expect(sidebar).toBeInstanceOf(HTMLElement);
    expect(within(sidebar as HTMLElement).getByRole("tab", { name: "Codex" }))
      .toHaveClass("is-active");
    expect(within(sidebar as HTMLElement).getByRole("tab", { name: /Claude/ }))
      .toBeDisabled();

    const row = within(sidebar as HTMLElement).getByRole("button", {
      name: /优化导入对话界面/,
    });
    expect(row).toHaveTextContent("优化导入对话界面");
    expect(row).not.toHaveTextContent("这段对话内容不应该出现在左侧列表");
    expect(row).not.toHaveTextContent("gpt-5.4");
    expect(row).not.toHaveTextContent("codex");

    const previewHead = view.container.querySelector(".conversation-import__preview-head");
    expect(previewHead).toBeInstanceOf(HTMLElement);
    expect(within(previewHead as HTMLElement).getByRole("button", { name: "导入到收集箱" }))
      .toBeEnabled();
    const scroller = mockScrollablePreview(view);
    await fireEvent.scroll(scroller);
    await waitFor(() => {
      expect(view.container.querySelector(".chat-scroll-map")).toBeInTheDocument();
    });
    expect(previewCodexThread).toHaveBeenCalledWith({ threadId: "thread-1", detail: "full" });
  });
});
