import { fireEvent, render, screen, waitFor } from "@testing-library/vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import MemoryView from "../src/pages/project/MemoryView.vue";

const memoryService = vi.hoisted(() => ({
  deleteMemory: vi.fn(),
  getMemorySettings: vi.fn(),
  listMemories: vi.fn(),
  setMemoryEnabled: vi.fn(),
  setMemorySettings: vi.fn(),
  upsertMemory: vi.fn(),
}));

vi.mock("../src/services/memory", () => memoryService);

const now = 1_720_000_000_000;

function memory(overrides: Record<string, unknown> = {}) {
  return {
    id: "memory-1",
    scope: "project",
    projectId: "project-1",
    title: "迁移检查",
    body: "DB 迁移必须先 dry-run。",
    tags: ["database"],
    enabled: true,
    sourceTaskId: null,
    createdAt: now,
    updatedAt: now,
    ...overrides,
  };
}

describe("MemoryView", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    memoryService.listMemories.mockResolvedValue([
      memory(),
      memory({
        id: "memory-user-1",
        scope: "user",
        projectId: null,
        title: "PR 文案",
        body: "PR 描述不要 emoji。",
      }),
    ]);
    memoryService.getMemorySettings.mockResolvedValue({
      enabled: true,
      baselineInjectionEnabled: true,
      cooldownTurns: 5,
    });
    memoryService.setMemorySettings.mockResolvedValue(undefined);
    memoryService.upsertMemory.mockImplementation(async (input) => memory({
      id: input.id ?? "memory-new",
      scope: input.scope,
      projectId: input.projectId,
      title: input.title,
      body: input.body,
      tags: input.tags ?? [],
      enabled: input.enabled !== false,
      updatedAt: now + 1,
    }));
    memoryService.setMemoryEnabled.mockImplementation(async (id, enabled) =>
      memory({ id, enabled, updatedAt: now + 1 })
    );
    memoryService.deleteMemory.mockResolvedValue(true);
  });

  it("loads user and project memories", async () => {
    render(MemoryView, { props: { projectId: "project-1" } });

    expect(await screen.findByText("迁移检查")).toBeTruthy();
    expect(screen.getByText("PR 文案")).toBeTruthy();
    expect(memoryService.listMemories).toHaveBeenCalledWith("project-1");
    expect(screen.getByDisplayValue("5")).toBeTruthy();
  });

  it("saves a project memory from the editor", async () => {
    render(MemoryView, { props: { projectId: "project-1" } });
    await screen.findByText("迁移检查");

    await fireEvent.update(screen.getByLabelText("标题"), "发布约束");
    await fireEvent.update(screen.getByLabelText("正文"), "发布前先跑 contracts。");
    await fireEvent.update(screen.getByLabelText("标签"), "release, check");
    await fireEvent.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(memoryService.upsertMemory).toHaveBeenCalledWith({
        id: null,
        scope: "project",
        projectId: "project-1",
        title: "发布约束",
        body: "发布前先跑 contracts。",
        tags: ["release", "check"],
        enabled: true,
        sourceTaskId: null,
      });
    });
    expect(await screen.findByText("发布约束")).toBeTruthy();
  });

  it("toggles settings and memory enabled state, then deletes", async () => {
    render(MemoryView, { props: { projectId: "project-1" } });
    await screen.findByText("迁移检查");

    await fireEvent.click(screen.getByLabelText("基线注入"));
    expect(memoryService.setMemorySettings).toHaveBeenCalledWith({
      enabled: true,
      baselineInjectionEnabled: false,
      cooldownTurns: 5,
    });

    const disableButtons = screen.getAllByTitle("停用");
    await fireEvent.click(disableButtons[0]);
    expect(memoryService.setMemoryEnabled).toHaveBeenCalledWith("memory-user-1", false);

    const deleteButtons = screen.getAllByTitle("删除");
    await fireEvent.click(deleteButtons[0]);
    expect(memoryService.deleteMemory).toHaveBeenCalledWith("memory-1");
  });

  it("normalizes memory settings through the shared contract rules", async () => {
    memoryService.getMemorySettings.mockResolvedValueOnce({
      enabled: true,
      baselineInjectionEnabled: true,
      cooldownTurns: 0,
    });

    render(MemoryView, { props: { projectId: "project-1" } });
    await screen.findByText("迁移检查");

    const cooldownInput = screen.getByLabelText("冷却 turn");
    await fireEvent.update(cooldownInput, "2.8");
    await fireEvent.change(cooldownInput);

    await waitFor(() => {
      expect(memoryService.setMemorySettings).toHaveBeenCalledWith({
        enabled: true,
        baselineInjectionEnabled: true,
        cooldownTurns: 2,
      });
    });
  });
});
