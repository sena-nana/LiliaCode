import { fireEvent, render, screen, waitFor } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import { SB_MENU_POP_TRANSITION_MS } from "../src/composables/menuMotion";
import { defineComponent, ref } from "vue";
import Dropdown from "../src/components/Dropdown.vue";

const options = [
  { value: "readonly", label: "只读", hint: "禁止写操作" },
  { value: "workspace-write", label: "工作区写入", hint: "允许编辑工作区" },
] as const;

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

  it("会从触发点击位置展开", async () => {
    const originalGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;
    Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", {
      configurable: true,
      value: function mockRect(this: HTMLElement) {
        if (this.classList.contains("dd")) {
          return {
            x: 100,
            y: 200,
            left: 100,
            top: 200,
            right: 220,
            bottom: 232,
            width: 120,
            height: 32,
            toJSON: () => ({}),
          } as DOMRect;
        }
        if (this.classList.contains("chat-chip")) {
          return {
            x: 100,
            y: 200,
            left: 100,
            top: 200,
            right: 220,
            bottom: 232,
            width: 120,
            height: 32,
            toJSON: () => ({}),
          } as DOMRect;
        }
        if (this.classList.contains("dd__menu")) {
          return {
            x: 100,
            y: 238,
            left: 100,
            top: 238,
            right: 280,
            bottom: 318,
            width: 180,
            height: 80,
            toJSON: () => ({}),
          } as DOMRect;
        }
        return originalGetBoundingClientRect.call(this);
      },
    });

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
      Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", {
        configurable: true,
        value: originalGetBoundingClientRect,
      });
    }
  });

  it("向上展开时会从触发边缘缩放", async () => {
    const originalGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;
    const originalOffsetWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetWidth");
    const originalOffsetHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetHeight");
    Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", {
      configurable: true,
      value: function mockRect(this: HTMLElement) {
        if (this.classList.contains("dd")) {
          return {
            x: 100,
            y: 200,
            left: 100,
            top: 200,
            right: 220,
            bottom: 232,
            width: 120,
            height: 32,
            toJSON: () => ({}),
          } as DOMRect;
        }
        if (this.classList.contains("chat-chip")) {
          return {
            x: 100,
            y: 200,
            left: 100,
            top: 200,
            right: 220,
            bottom: 232,
            width: 120,
            height: 32,
            toJSON: () => ({}),
          } as DOMRect;
        }
        if (this.classList.contains("dd__menu")) {
          return {
            x: 100,
            y: 114,
            left: 100,
            top: 114,
            right: 280,
            bottom: 194,
            width: 180,
            height: 80,
            toJSON: () => ({}),
          } as DOMRect;
        }
        return originalGetBoundingClientRect.call(this);
      },
    });
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
    }
  });

  it("底部空间不足时会翻转为向上展开语义", async () => {
    const originalGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;
    const originalOffsetWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetWidth");
    const originalOffsetHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetHeight");
    const originalInnerHeight = window.innerHeight;
    Object.defineProperty(window, "innerHeight", {
      configurable: true,
      value: 260,
    });
    Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", {
      configurable: true,
      value: function mockRect(this: HTMLElement) {
        if (this.classList.contains("dd")) {
          return {
            x: 100,
            y: 200,
            left: 100,
            top: 200,
            right: 220,
            bottom: 232,
            width: 120,
            height: 32,
            toJSON: () => ({}),
          } as DOMRect;
        }
        if (this.classList.contains("chat-chip")) {
          return {
            x: 100,
            y: 200,
            left: 100,
            top: 200,
            right: 220,
            bottom: 232,
            width: 120,
            height: 32,
            toJSON: () => ({}),
          } as DOMRect;
        }
        if (this.classList.contains("dd__menu")) {
          return {
            x: 100,
            y: 114,
            left: 100,
            top: 114,
            right: 280,
            bottom: 194,
            width: 180,
            height: 80,
            toJSON: () => ({}),
          } as DOMRect;
        }
        return originalGetBoundingClientRect.call(this);
      },
    });
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
      Object.defineProperty(window, "innerHeight", {
        configurable: true,
        value: originalInnerHeight,
      });
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
    }
  });
});
