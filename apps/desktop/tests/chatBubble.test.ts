import { fireEvent, render } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import type { ChatMessage } from "@lilia/contracts";
import ChatBubble from "../src/components/chat/ChatBubble.vue";

function message(overrides: Partial<ChatMessage>): ChatMessage {
  return {
    id: "msg-1",
    taskId: "task-1",
    role: "user",
    content: "",
    attachments: [],
    createdAt: 1,
    ...overrides,
  };
}

describe("ChatBubble", () => {
  it("图片附件只显示缩略图，普通附件保留名称", () => {
    const view = render(ChatBubble, {
      props: {
        message: message({
          attachments: [
            {
              id: "image-1",
              name: "图片 1.png",
              path: "D:\\PROJECT\\workspace\\Lilia\\shot.png",
              kind: "file",
              size: 42,
              exists: true,
              mime: "image/png",
              directory: null,
            },
            {
              id: "file-1",
              name: "README.md",
              path: "D:\\PROJECT\\workspace\\Lilia\\README.md",
              kind: "file",
              size: 42,
              exists: true,
              mime: null,
              directory: null,
            },
          ],
        }),
      },
    });

    const attachments = view.getByLabelText("消息附件");
    const imageChip = attachments.querySelector(".chat-attachment-chip--image-preview");

    expect(imageChip).not.toBeNull();
    expect(imageChip?.querySelector(".chat-attachment-chip__thumb")).not.toBeNull();
    expect(imageChip?.querySelector(".chat-attachment-chip__name")).toBeNull();
    expect(attachments).not.toHaveTextContent("图片 1.png");
    expect(attachments).toHaveTextContent("README.md");
  });

  it("点击图片附件会请求打开图片查看器", async () => {
    const view = render(ChatBubble, {
      props: {
        message: message({
          attachments: [
            {
              id: "image-1",
              name: "图片 1.png",
              path: "D:\\PROJECT\\workspace\\Lilia\\shot.png",
              kind: "file",
              size: 42,
              exists: true,
              mime: "image/png",
              directory: null,
            },
          ],
        }),
      },
    });

    await fireEvent.click(view.getByRole("button", { name: "查看图片 图片 1.png" }));

    expect(view.emitted("open-image")?.[0]?.[0]).toMatchObject({
      src: "asset://D:/PROJECT/workspace/Lilia/shot.png",
      name: "图片 1.png",
      path: "D:\\PROJECT\\workspace\\Lilia\\shot.png",
      mime: "image/png",
      size: 42,
    });
  });
});
