import { fireEvent, render, screen, waitFor } from "@testing-library/vue";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import ContextMenuHost from "../src/components/ContextMenuHost.vue";
import {
  closeContextMenu,
  openContextMenuAt,
} from "../src/composables/useContextMenu";
import { installContextMenu } from "../src/composables/useContextMenuInstall";
import { SB_MENU_POP_TRANSITION_MS } from "../src/composables/menuMotion";

async function waitForMenuLeave() {
  await vi.advanceTimersByTimeAsync(SB_MENU_POP_TRANSITION_MS + 50);
}

describe("ContextMenuHost", () => {
  beforeAll(() => {
    installContextMenu();
  });

  afterEach(async () => {
    closeContextMenu();
    vi.useRealTimers();
  });

  it("打开后可渲染菜单项，并在 Escape 后完成移除", async () => {
    vi.useFakeTimers();
    render(ContextMenuHost, {
      global: {
        stubs: {
          transition: false,
        },
      },
    });

    openContextMenuAt(48, 56, [{
      id: "open-project",
      label: "进入项目",
      onSelect: () => {},
    }]);

    expect(await screen.findByRole("menuitem", { name: "进入项目" })).toBeInTheDocument();

    await fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.getByRole("menuitem", { name: "进入项目" })).toBeInTheDocument();

    await waitForMenuLeave();
    await waitFor(() => {
      expect(screen.queryByRole("menuitem", { name: "进入项目" })).not.toBeInTheDocument();
    });
  });

  it("退场期间重新打开时应显示新的菜单内容", async () => {
    vi.useFakeTimers();
    render(ContextMenuHost, {
      global: {
        stubs: {
          transition: false,
        },
      },
    });

    openContextMenuAt(48, 56, [{
      id: "old-item",
      label: "旧菜单",
      onSelect: () => {},
    }]);
    expect(await screen.findByRole("menuitem", { name: "旧菜单" })).toBeInTheDocument();

    closeContextMenu();
    openContextMenuAt(92, 88, [{
      id: "new-item",
      label: "新菜单",
      onSelect: () => {},
    }]);

    expect(await screen.findByRole("menuitem", { name: "新菜单" })).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "旧菜单" })).not.toBeInTheDocument();

    await waitForMenuLeave();
    expect(screen.getByRole("menuitem", { name: "新菜单" })).toBeInTheDocument();
  });
});
