import { fireEvent, render, waitFor, within } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import SubagentCatalogSection from "../src/pages/settings/SubagentCatalogSection.vue";
import { mockInvoke } from "./tauriMock";

function lastInvokeInput(command: string): Record<string, unknown> | undefined {
  const call = [...mockInvoke.mock.calls].reverse().find(([cmd]) => cmd === command);
  const input = call?.[1];
  return input && typeof input === "object" ? input as Record<string, unknown> : undefined;
}

describe("SubagentCatalogSection", () => {
  it("supports creating, editing, toggling, and deleting custom subagents", async () => {
    const view = render(SubagentCatalogSection);

    expect(await view.findByText("自定义 Agent")).toBeInTheDocument();
    expect(await view.findByText("Reviewer")).toBeInTheDocument();

    await fireEvent.click(view.getByRole("button", { name: "新建 Agent" }));
    await fireEvent.update(view.getByLabelText("名称"), "Builder");
    await fireEvent.update(view.getByLabelText("描述"), "处理实现细节");
    await fireEvent.update(view.getByLabelText("职责说明"), "Implement the requested code changes.");
    await fireEvent.click(view.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(lastInvokeInput("agent_interaction_upsert_subagent")).toMatchObject({
        input: {
          name: "Builder",
          description: "处理实现细节",
          instruction: "Implement the requested code changes.",
          enabled: true,
        },
      });
    });
    expect(await view.findByText("Builder")).toBeInTheDocument();

    let builderItem = view.getByText("Builder").closest(".subagent-item") as HTMLElement;
    await fireEvent.click(within(builderItem).getByRole("button", { name: "停用" }));

    await waitFor(() => {
      expect(lastInvokeInput("agent_interaction_upsert_subagent")).toMatchObject({
        input: {
          name: "Builder",
          enabled: false,
        },
      });
    });
    await waitFor(() => {
      expect(view.getByText("Builder").closest(".subagent-item")?.textContent).toContain("已停用");
    });

    builderItem = view.getByText("Builder").closest(".subagent-item") as HTMLElement;
    await fireEvent.click(within(builderItem).getByRole("button", { name: "编辑" }));
    await fireEvent.update(view.getByLabelText("描述"), "处理实现与重构");
    await fireEvent.click(view.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(lastInvokeInput("agent_interaction_upsert_subagent")).toMatchObject({
        input: {
          name: "Builder",
          description: "处理实现与重构",
          enabled: false,
        },
      });
    });

    builderItem = view.getByText("Builder").closest(".subagent-item") as HTMLElement;
    await fireEvent.click(within(builderItem).getByRole("button", { name: "删除" }));

    await waitFor(() => {
      expect(lastInvokeInput("agent_interaction_delete_subagent")).toMatchObject({
        id: expect.any(String),
      });
    });
    await waitFor(() => {
      expect(view.queryByText("Builder")).not.toBeInTheDocument();
    });
  });
});
