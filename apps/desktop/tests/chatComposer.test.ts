import { fireEvent, render } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import type { ChatComposerState } from "@lilia/contracts";
import ChatComposer from "../src/components/chat/ChatComposer.vue";

const baseState: ChatComposerState = {
  taskId: "task-1",
  backend: "claude",
  model: "claude-sonnet-4-6",
  branch: "main",
  planMode: false,
  permission: "full",
};

describe("ChatComposer", () => {
  it("计划模式独立于执行权限切换", async () => {
    const view = render(ChatComposer, {
      props: {
        state: baseState,
        branches: [{ name: "main", current: true }],
        attachments: [],
      },
    });

    const planButton = view.getByRole("button", { name: "开启计划模式" });
    expect(planButton).toHaveAttribute("aria-pressed", "false");

    await fireEvent.click(planButton);

    expect(view.emitted("update:state")?.[0]?.[0]).toMatchObject({
      planMode: true,
      permission: "full",
    });
  });
});
