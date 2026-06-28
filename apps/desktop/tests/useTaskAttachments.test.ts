import { render } from "@testing-library/vue";
import { describe, expect, it, vi } from "vitest";
import { defineComponent, h, ref } from "vue";
import type { ChatAttachment } from "@lilia/contracts";
import { useTaskAttachments } from "../src/pages/taskDetail/useTaskAttachments";
import { describeAttachments } from "../src/services/chat";

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    scaleFactor: vi.fn(async () => 1),
  }),
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(async () => vi.fn()),
  }),
}));

vi.mock("../src/services/chat", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/services/chat")>();
  return {
    ...actual,
    describeAttachments: vi.fn(),
  };
});

describe("useTaskAttachments", () => {
  it("卸载后忽略仍在返回的附件描述结果", async () => {
    let resolveDescribe: (attachments: ChatAttachment[]) => void = () => {};
    vi.mocked(describeAttachments).mockReturnValue(
      new Promise((resolve) => {
        resolveDescribe = resolve;
      }),
    );
    let attachments: ReturnType<typeof useTaskAttachments>["attachments"] | null = null;

    const Harness = defineComponent({
      setup() {
        const chatPageRef = ref<HTMLElement | null>(null);
        const controller = useTaskAttachments({
          chatPageRef,
          hasContext: () => true,
          canAcceptInteractiveDrop: () => true,
        });
        attachments = controller.attachments;
        void controller.addAttachmentsFromPaths(["D:\\PROJECT\\workspace\\Lilia\\late.png"]);
        return () => h("div", { ref: chatPageRef });
      },
    });

    const view = render(Harness);
    view.unmount();
    resolveDescribe([{
      id: "late",
      name: "late.png",
      path: "D:\\PROJECT\\workspace\\Lilia\\late.png",
      kind: "image",
      size: 120,
      exists: true,
      mime: "image/png",
    }]);
    await Promise.resolve();

    expect(attachments?.value).toEqual([]);
  });
});

