import { render, screen, waitFor, within } from "@testing-library/vue";
import { describe, expect, it, vi } from "vitest";
import ArchitectureSidebarPanel from "../src/components/chat/ArchitectureSidebarPanel.vue";
import {
  getProjectArchitecture,
  listProjectArchitectureChanges,
} from "../src/services/chat";
import type {
  ProjectArchitectureChangeRecord,
  ProjectArchitectureGraph,
} from "@lilia/contracts";

vi.mock("../src/components/chat/MarkdownMermaid.vue", () => ({
  default: { template: "<div data-testid=\"mermaid\"></div>" },
}));

vi.mock("../src/composables/useConnectionStatus", () => ({
  useConnectionStatus: () => ({ activeBackend: { value: "codex" } }),
}));

vi.mock("../src/services/chat", () => ({
  getProjectArchitecture: vi.fn(),
  listProjectArchitectureChanges: vi.fn(),
  rollbackProjectArchitecture: vi.fn(),
}));

const graph: ProjectArchitectureGraph = {
  projectId: "p1",
  version: 4,
  summary: "桌面端架构",
  nodes: [
    {
      id: "ui",
      label: "UI",
      type: "component",
      summary: "",
      paths: [],
      tags: [],
    },
  ],
  edges: [],
  updatedAt: 1_735_689_600_000,
};

const rollbackCreatedAt = 1_735_696_800_000;
const expectedRollbackTime = new Intl.DateTimeFormat("zh-CN", {
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
}).format(new Date(rollbackCreatedAt));

function changeRecord(
  patch: Partial<ProjectArchitectureChangeRecord>,
): ProjectArchitectureChangeRecord {
  return {
    id: "change-1",
    projectId: "p1",
    taskId: "t1",
    turnId: null,
    backend: "codex",
    permission: "full",
    status: "applied",
    reason: "补全 UI 节点",
    changes: [],
    beforeVersion: 1,
    afterVersion: 2,
    createdAt: 1_735_693_200_000,
    resolvedAt: 1_735_693_200_000,
    beforeGraph: null,
    afterGraph: null,
    ...patch,
  };
}

describe("ArchitectureSidebarPanel", () => {
  it("最近变更展示来源、时间、版本变化和回滚标识", async () => {
    vi.mocked(getProjectArchitecture).mockResolvedValue(graph);
    vi.mocked(listProjectArchitectureChanges).mockResolvedValue([
      changeRecord({ id: "change-2", backend: "claude", beforeVersion: 2, afterVersion: 3 }),
      changeRecord({
        id: "rollback-1",
        status: "rolled_back",
        reason: "回滚到上一版本",
        beforeVersion: 3,
        afterVersion: 4,
        createdAt: rollbackCreatedAt,
      }),
    ]);

    render(ArchitectureSidebarPanel, {
      props: {
        taskId: "t1",
        projectId: "p1",
        projectCwd: "C:\\Files\\workspace\\Lilia",
      },
    });

    const section = await screen.findByRole("region", { name: "最近变更" });

    await waitFor(() => {
      expect(within(section).getByText("backend：claude")).toBeInTheDocument();
      expect(within(section).getByText("版本：v2 -> v3")).toBeInTheDocument();
      expect(within(section).getByText("rolled_back")).toBeInTheDocument();
      expect(within(section).getByText("版本：v3 -> v4")).toBeInTheDocument();
    });
    expect(section).toHaveTextContent("回滚到上一版本");
    expect(section).toHaveTextContent(`时间：${expectedRollbackTime}`);
    expect(section).toHaveTextContent("backend：codex");
  });

  it("项目切换时忽略较早返回的架构加载结果", async () => {
    const nextGraph: ProjectArchitectureGraph = {
      ...graph,
      projectId: "p2",
      version: 9,
      summary: "新项目架构",
    };
    let resolveOldGraph: (value: ProjectArchitectureGraph) => void = () => {};
    vi.mocked(getProjectArchitecture).mockImplementation((projectId) => {
      if (projectId === "p2") return Promise.resolve(nextGraph);
      return new Promise((resolve) => {
        resolveOldGraph = resolve;
      });
    });
    vi.mocked(listProjectArchitectureChanges).mockResolvedValue([]);

    const rendered = render(ArchitectureSidebarPanel, {
      props: {
        taskId: "t1",
        projectId: "p1",
        projectCwd: "C:\\Files\\workspace\\Lilia",
      },
    });

    expect(await screen.findByText("正在加载架构图…")).toBeInTheDocument();

    await rendered.rerender({
      taskId: "t1",
      projectId: "p2",
      projectCwd: "C:\\Files\\workspace\\Lilia",
    });

    expect(await screen.findByText("新项目架构")).toBeInTheDocument();
    expect(screen.getByText("版本 9")).toBeInTheDocument();

    resolveOldGraph({
      ...graph,
      summary: "旧项目架构",
    });

    await waitFor(() => {
      expect(screen.queryByText("旧项目架构")).not.toBeInTheDocument();
      expect(screen.getByText("版本 9")).toBeInTheDocument();
    });
  });

  it("卸载后忽略仍在返回的架构加载结果", async () => {
    let resolveGraph: (value: ProjectArchitectureGraph) => void = () => {};
    vi.mocked(getProjectArchitecture).mockReturnValue(
      new Promise((resolve) => {
        resolveGraph = resolve;
      }),
    );
    vi.mocked(listProjectArchitectureChanges).mockResolvedValue([]);

    const rendered = render(ArchitectureSidebarPanel, {
      props: {
        taskId: "t1",
        projectId: "p1",
        projectCwd: "C:\\Files\\workspace\\Lilia",
      },
    });

    expect(await screen.findByText("正在加载架构图…")).toBeInTheDocument();
    rendered.unmount();
    resolveGraph(graph);

    await Promise.resolve();
  });
});

