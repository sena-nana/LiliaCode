import { fireEvent, render } from "@testing-library/vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AppShell from "../src/layouts/AppShell.vue";

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isMaximized: vi.fn(async () => false),
    onResized: vi.fn(async () => vi.fn()),
    minimize: vi.fn(async () => undefined),
    toggleMaximize: vi.fn(async () => undefined),
    close: vi.fn(async () => undefined),
  }),
}));

const COLLAPSED_STORAGE_KEY = "lilia.sidebarCollapsed";

async function renderAppShell() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/:pathMatch(.*)*", component: { template: "<div />" } }],
  });
  await router.push("/");
  await router.isReady();

  return render(AppShell, {
    global: {
      plugins: [router],
    },
  });
}

function shellElement(container: HTMLElement): HTMLElement {
  const shell = container.querySelector(".shell");
  if (!(shell instanceof HTMLElement)) {
    throw new Error("未找到 shell");
  }
  return shell;
}

function leftResizer(container: HTMLElement): HTMLElement {
  const resizer = container.querySelector(".shell__resizer");
  if (!(resizer instanceof HTMLElement)) {
    throw new Error("未找到左侧栏拖拽线");
  }
  return resizer;
}

beforeEach(() => {
  localStorage.clear();
});

describe("AppShell left sidebar collapse", () => {
  it("左上角按钮切换左侧栏折叠状态并写回本地存储", async () => {
    const view = await renderAppShell();
    const shell = shellElement(view.container);
    const resizer = leftResizer(view.container);
    const collapse = view.getByRole("button", { name: "折叠左侧栏" });

    expect(shell).not.toHaveClass("is-sidebar-collapsed");
    expect(resizer).not.toHaveAttribute("aria-disabled");
    expect(collapse).toHaveAttribute("aria-pressed", "false");

    await fireEvent.click(collapse);

    expect(shell).toHaveClass("is-sidebar-collapsed");
    expect(leftResizer(view.container)).toHaveAttribute("aria-disabled", "true");
    expect(localStorage.getItem(COLLAPSED_STORAGE_KEY)).toBe("1");

    const expand = view.getByRole("button", { name: "展开左侧栏" });
    expect(expand).toHaveAttribute("aria-pressed", "true");

    await fireEvent.click(expand);

    expect(shell).not.toHaveClass("is-sidebar-collapsed");
    expect(leftResizer(view.container)).not.toHaveAttribute("aria-disabled");
    expect(localStorage.getItem(COLLAPSED_STORAGE_KEY)).toBe("0");
  });

  it("会从本地存储恢复左侧栏折叠状态", async () => {
    localStorage.setItem(COLLAPSED_STORAGE_KEY, "1");

    const view = await renderAppShell();

    expect(shellElement(view.container)).toHaveClass("is-sidebar-collapsed");
    expect(view.getByRole("button", { name: "展开左侧栏" }))
      .toHaveAttribute("aria-pressed", "true");
    expect(leftResizer(view.container)).toHaveAttribute("aria-disabled", "true");
  });
});
