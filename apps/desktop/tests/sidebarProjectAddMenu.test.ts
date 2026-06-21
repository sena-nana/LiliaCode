import { fireEvent, render } from "@testing-library/vue";
import { describe, expect, it, vi } from "vitest";
import SidebarProjectAddMenu from "../src/components/sidebar/SidebarProjectAddMenu.vue";
import { pickFolder } from "../src/services/projects";
import { createProject } from "../src/services/projectsStore";

vi.mock("../src/services/projects", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/services/projects")>();
  return {
    ...actual,
    pickFolder: vi.fn(),
  };
});

vi.mock("../src/services/projectsStore", () => ({
  createProject: vi.fn(),
  deriveProjectName: (path: string) => path.split(/[\\/]/).pop() ?? "",
}));

describe("SidebarProjectAddMenu", () => {
  it("卸载后忽略仍在返回的本地文件夹选择结果", async () => {
    let resolvePickFolder: (path: string) => void = () => {};
    vi.mocked(pickFolder).mockReturnValue(
      new Promise((resolve) => {
        resolvePickFolder = resolve;
      }),
    );
    vi.mocked(createProject).mockResolvedValue({
      id: "p-late",
      name: "Late",
      cwd: "D:\\PROJECT\\workspace\\Late",
      sessionCount: 0,
      pinned: false,
    });
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

    expect(createProject).not.toHaveBeenCalled();
    expect(onCreated).not.toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();
  });
});
