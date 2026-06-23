import { fireEvent, render, screen, waitFor } from "@testing-library/vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SB_MENU_POP_TRANSITION_MS } from "../src/composables/menuMotion";
import { defineComponent, ref } from "vue";
import Dropdown from "../src/components/Dropdown.vue";
import { domRect } from "./domTestHelpers";

const options = [
  { value: "readonly", label: "只读", hint: "禁止写操作" },
  { value: "workspace-write", label: "工作区写入", hint: "允许编辑工作区" },
] as const;

afterEach(() => {
  vi.restoreAllMocks();
});

function renderDropdown() {
  return render(defineComponent({
    components: { Dropdown },
    setup() {
      const value = ref<"readonly" | "workspace-write">("readonly");
      return { options, value };
    },
    template: `
      <Dropdown
        v-model="value"
        :options="options"
        placement="bottom"
      />
    `,
  }), {
    global: {
      stubs: {
        transition: false,
      },
    },
  });
}

function installDropdownGeometryMock({
  menuTop = 238,
  innerHeight,
  mockMenuSize = false,
}: {
  menuTop?: number;
  innerHeight?: number;
  mockMenuSize?: boolean;
} = {}) {
  const originalGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;
  const originalOffsetWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetWidth");
  const originalOffsetHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetHeight");
  const originalInnerHeight = window.innerHeight;
  if (innerHeight !== undefined) {
    Object.defineProperty(window, "innerHeight", { configurable: true, value: innerHeight });
  }
  Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", {
    configurable: true,
      value: function mockRect(this: HTMLElement) {
      if (this.classList.contains("dd") || this.classList.contains("chat-chip")) {
        return domRect(100, 200, 120, 32);
      }
      if (this.classList.contains("dd__menu")) {
        return domRect(100, menuTop, 180, 80);
      }
      return originalGetBoundingClientRect.call(this);
    },
  });
  if (mockMenuSize) {
    Object.defineProperty(HTMLElement.prototype, "offsetWidth", {
      configurable: true,
      get() {
        return this.classList.contains("dd__menu") ? 180 : 0;
      },
    });
    Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
      configurable: true,
      get() {
        return this.classList.contains("dd__menu") ? 80 : 0;
      },
    });
  }
  return () => {
    if (innerHeight !== undefined) {
      Object.defineProperty(window, "innerHeight", { configurable: true, value: originalInnerHeight });
    }
    Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", {
      configurable: true,
      value: originalGetBoundingClientRect,
    });
    if (originalOffsetWidth) {
      Object.defineProperty(HTMLElement.prototype, "offsetWidth", originalOffsetWidth);
    }
    if (originalOffsetHeight) {
      Object.defineProperty(HTMLElement.prototype, "offsetHeight", originalOffsetHeight);
    }
  };
}

describe("Dropdown", () => {
  it("可打开、选中选项并关闭", async () => {
    renderDropdown();

    await fireEvent.click(screen.getByRole("button", { name: /只读/i }));
    expect(await screen.findByRole("listbox")).toBeInTheDocument();
    expect(await screen.findByRole("option", { name: /工作区写入/i })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("option", { name: /工作区写入/i }));

    await waitFor(
      () => {
        expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      },
      { timeout: SB_MENU_POP_TRANSITION_MS + 400 },
    );
    expect(screen.getByRole("button", { name: /工作区写入/i })).toBeInTheDocument();
  });

  it("teleports to body and closes on outside click and Escape", async () => {
    const view = renderDropdown();

    await fireEvent.click(screen.getByRole("button", { name: /只读/i }));

    const listbox = await screen.findByRole("listbox");
    expect(document.body.contains(listbox)).toBe(true);
    expect(view.container.contains(listbox)).toBe(false);
    await Promise.resolve();
    await Promise.resolve();

    const outside = document.createElement("button");
    document.body.appendChild(outside);
    await fireEvent.pointerDown(outside);
    outside.remove();

    await waitFor(() => {
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    });

    await fireEvent.click(screen.getByRole("button", { name: /只读/i }));
    expect(await screen.findByRole("listbox")).toBeInTheDocument();
    await Promise.resolve();
    await Promise.resolve();

    await fireEvent.keyDown(document, { key: "Escape" });

    await waitFor(() => {
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    });
  });

  it("快速关闭时不会在异步定位后补装 document listener", async () => {
    const addListenerSpy = vi.spyOn(document, "addEventListener");
    renderDropdown();
    const trigger = screen.getByRole("button", { name: /只读/i });

    await fireEvent.click(trigger);
    await fireEvent.click(trigger);
    await Promise.resolve();
    await Promise.resolve();

    expect(addListenerSpy).not.toHaveBeenCalledWith("pointerdown", expect.any(Function), true);
    expect(addListenerSpy).not.toHaveBeenCalledWith("keydown", expect.any(Function));
  });

  it("会从触发点击位置展开", async () => {
    const restoreGeometry = installDropdownGeometryMock();

    try {
      renderDropdown();
      await fireEvent.click(screen.getByRole("button", { name: /只读/i }), {
        clientX: 136,
        clientY: 216,
      });

      const listbox = await screen.findByRole("listbox");
      expect(listbox).toHaveStyle({
        "--sb-menu-origin-x": "36px",
        "--sb-menu-origin-y": "0px",
      });
    } finally {
      restoreGeometry();
    }
  });

  it("向上展开时会从触发边缘缩放", async () => {
    const restoreGeometry = installDropdownGeometryMock({ menuTop: 114, mockMenuSize: true });

    try {
      render(defineComponent({
        components: { Dropdown },
        setup() {
          const value = ref<"readonly" | "workspace-write">("readonly");
          return { options, value };
        },
        template: `
          <Dropdown
            v-model="value"
            :options="options"
          />
        `,
      }), {
        global: {
          stubs: {
            transition: false,
          },
        },
      });

      await fireEvent.click(screen.getByRole("button", { name: /只读/i }), {
        clientX: 136,
        clientY: 216,
      });

      const listbox = await screen.findByRole("listbox");
      await waitFor(() => {
        expect(listbox).toHaveStyle({
          "--sb-menu-origin-x": "36px",
          "--sb-menu-origin-y": "80px",
        });
      });
    } finally {
      restoreGeometry();
    }
  });

  it("底部空间不足时会翻转为向上展开语义", async () => {
    const restoreGeometry = installDropdownGeometryMock({
      menuTop: 114,
      innerHeight: 260,
      mockMenuSize: true,
    });

    try {
      renderDropdown();
      await fireEvent.click(screen.getByRole("button", { name: /只读/i }), {
        clientX: 136,
        clientY: 216,
      });

      const listbox = await screen.findByRole("listbox");
      await waitFor(() => {
        expect(listbox).toHaveClass("dd__menu--top");
        expect(listbox).toHaveStyle({ top: "114px" });
      });
    } finally {
      restoreGeometry();
    }
  });
});
