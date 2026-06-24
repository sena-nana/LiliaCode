import { describe, expect, it, vi } from "vitest";
import type { Router } from "vue-router";
import type { SuggestionItem } from "@lilia/contracts";
import {
  loadTaskDetailSuggestions,
  moveTaskDetailDraftToProject,
} from "../src/pages/taskDetail/taskDetailDeferredUi";

const mocks = vi.hoisted(() => ({
  getConversationSuggestionSources: vi.fn(async () => ({
    sources: ["task"],
    localGit: null,
  })),
  getConversationSuggestions: vi.fn(async (): Promise<SuggestionItem[]> => [
    {
      id: "sg-stale",
      projectId: "lilia",
      taskIds: [],
      source: "task",
      githubActivities: [],
      localGitContexts: [],
      codexThreads: [],
      summary: "过期建议",
      reason: "旧请求晚返回。",
      prompt: "不应写回。",
      generatedAt: Date.now(),
    },
  ]),
  createDraftTask: vi.fn((projectId: string) => ({
    id: `draft-${projectId}`,
  })),
}));

vi.mock("../src/services/chat", () => ({
  getConversationSuggestionSources: mocks.getConversationSuggestionSources,
  getConversationSuggestions: mocks.getConversationSuggestions,
}));

vi.mock("../src/services/tasksStore", () => ({
  createDraftTask: mocks.createDraftTask,
}));

vi.mock("../src/services/projectsStore", () => ({
  listProjects: vi.fn(async () => []),
}));

describe("taskDetailDeferredUi", () => {
  it("建议请求过期后不再写入结果状态", async () => {
    const setSuggestions = vi.fn();
    const setStatus = vi.fn();
    const setLoadingText = vi.fn();

    await loadTaskDetailSuggestions({
      detail: "lilia:t-draft",
      projectId: "lilia",
      forceRefresh: false,
      shouldLoad: true,
      seq: 1,
      isCurrentSeq: () => false,
      setSuggestions,
      setStatus,
      setLoadingText,
    });

    expect(mocks.getConversationSuggestionSources).toHaveBeenCalledWith("lilia", false);
    expect(mocks.getConversationSuggestions).not.toHaveBeenCalled();
    expect(setStatus).toHaveBeenCalledWith("loading");
    expect(setStatus).not.toHaveBeenCalledWith("idle");
    expect(setSuggestions).not.toHaveBeenCalled();
  });

  it("源草稿页失效后不创建目标草稿", async () => {
    const router = { push: vi.fn() } as unknown as Router;

    await moveTaskDetailDraftToProject({
      detail: "orphan:t-draft",
      projectId: "lilia",
      attachments: [],
      router,
      isSourceCurrent: () => false,
      isTargetCurrent: () => true,
      getDraftSnapshot: vi.fn(() => ({ content: "旧草稿" })),
      restoreDraft: vi.fn(),
    });

    expect(mocks.createDraftTask).not.toHaveBeenCalled();
    expect(router.push).not.toHaveBeenCalled();
  });

  it("目标草稿页失效后不恢复旧草稿内容", async () => {
    const router = { push: vi.fn(async () => undefined) } as unknown as Router;
    const restoreDraft = vi.fn();

    await moveTaskDetailDraftToProject({
      detail: "orphan:t-draft",
      projectId: "lilia",
      attachments: [],
      router,
      isSourceCurrent: () => true,
      isTargetCurrent: () => false,
      getDraftSnapshot: vi.fn(() => ({ content: "旧草稿" })),
      restoreDraft,
    });

    expect(mocks.createDraftTask).toHaveBeenCalledWith("lilia");
    expect(router.push).toHaveBeenCalledWith("/projects/lilia/tasks/draft-lilia");
    expect(restoreDraft).not.toHaveBeenCalled();
  });
});
