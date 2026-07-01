import { fireEvent, render } from "@testing-library/vue";
import { describe, expect, it, vi } from "vitest";
import ComposerProjectPicker from "../src/components/chat/ComposerProjectPicker.vue";
import SidebarProjectAddMenu from "../src/components/sidebar/SidebarProjectAddMenu.vue";
import { pickFolder } from "../src/services/projects";
import { ensureFolderProjects } from "../src/services/projectsStore";

vi.mock("../src/services/projects", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/services/projects")>();
  return {
    ...actual,
    pickFolder: vi.fn(),
  };
});

vi.mock("../src/services/projectsStore", () => ({
  createProject: vi.fn(),
  ensureFolderProjects: vi.fn(),
}));

describe("SidebarProjectAddMenu", () => {
  it("侧栏入口保留空分类创建", () => {
    const view = render(SidebarProjectAddMenu, {
      props: {
        open: true,
        position: { x: 0, y: 0, anchorX: 0, anchorY: 0 },
      },
    });

    expect(view.getByRole("menuitem", { name: "创建空分类" })).toBeTruthy();
  });

  it("聊天项目选择器只暴露可绑定目录的项目来源", async () => {
    const view = render(ComposerProjectPicker, {
      props: {
        projects: [],
      },
    });

    await fireEvent.click(view.getByRole("button", { name: "打开新项目" }));

    expect(view.getByRole("menuitem", { name: "使用本地文件夹" })).toBeTruthy();
    expect(view.getByRole("menuitem", { name: "从 GitHub clone" })).toBeTruthy();
    expect(view.queryByRole("menuitem", { name: "创建空分类" })).toBeNull();
  });

  it("卸载后忽略仍在返回的本地文件夹选择结果", async () => {
    let resolvePickFolder: (path: string) => void = () => {};
    vi.mocked(pickFolder).mockReturnValue(
      new Promise((resolve) => {
        resolvePickFolder = resolve;
      }),
    );
    vi.mocked(ensureFolderProjects).mockResolvedValue([{
      id: "p-late",
      name: "Late",
      cwd: "D:\\PROJECT\\workspace\\Late",
      sessionCount: 0,
      pinned: false,
    }]);
    const onCreated = vi.fn();
    const onError = vi.fn();

    const view = render(SidebarProjectAddMenu, {
      props: {
        open: true,
        position: { x: 0, y: 0, anchorX: 0, anchorY: 0 },
        onCreated,
        onError,
      },
    });

    await fireEvent.click(await view.findByRole("menuitem", { name: "使用本地文件夹" }));
    view.unmount();
    resolvePickFolder("D:\\PROJECT\\workspace\\Late");
    await Promise.resolve();

    expect(ensureFolderProjects).not.toHaveBeenCalled();
    expect(onCreated).not.toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();
  });
});

