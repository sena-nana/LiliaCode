import { fireEvent, render, waitFor } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import Plugins from "../src/pages/Plugins.vue";
import { mockInvoke } from "./tauriMock";

function lastClaudeMcpUpdateInput() {
  const call = mockInvoke.mock.calls
    .filter(([command]) => command === "plugins_update_claude_mcp_server")
    .at(-1);
  return call?.[1]?.input;
}

describe("Plugins page", () => {
  it("展示并管理 Claude MCP server", async () => {
    const view = render(Plugins);

    await fireEvent.click(view.getByRole("tab", { name: /Claude MCP/ }));

    await waitFor(() => {
      expect(view.getByText("weather")).toBeInTheDocument();
    });
    expect(view.getByText("node weather-mcp.js")).toBeInTheDocument();
    expect(view.getByText("WEATHER_TOKEN")).toBeInTheDocument();

    await fireEvent.click(view.getByRole("button", { name: "停用" }));
    await waitFor(() => {
      expect(view.getByText("已停用")).toBeInTheDocument();
    });

    await fireEvent.click(view.getByRole("button", { name: /新增 MCP/ }));
    await fireEvent.update(view.getByPlaceholderText("weather-mcp"), "linear");
    await fireEvent.update(view.getByPlaceholderText("node"), "uvx");
    await fireEvent.update(view.getByPlaceholderText("每行一个参数"), "linear-mcp");
    await fireEvent.click(view.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(view.getByText("linear")).toBeInTheDocument();
    });
    expect(view.getByText("uvx linear-mcp")).toBeInTheDocument();
  });

  it("编辑 Claude MCP 时留空 env 保留旧值，删行才删除旧 key", async () => {
    const view = render(Plugins);

    await fireEvent.click(view.getByRole("tab", { name: /Claude MCP/ }));
    await waitFor(() => {
      expect(view.getByText("weather")).toBeInTheDocument();
    });

    await fireEvent.click(view.getByRole("button", { name: /编辑/ }));
    await fireEvent.click(view.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(lastClaudeMcpUpdateInput()).toEqual(
        expect.not.objectContaining({
          removeEnvKeys: expect.any(Array),
        }),
      );
    });
    await waitFor(() => {
      expect(view.queryByRole("dialog", { name: "Claude MCP" })).not.toBeInTheDocument();
    });

    mockInvoke.mockClear();
    await fireEvent.click(view.getByRole("button", { name: /编辑/ }));
    await waitFor(() => {
      expect(view.getByRole("button", { name: "保存" })).toBeInTheDocument();
    });
    await fireEvent.click(view.getByRole("button", { name: "删除 Env" }));
    await fireEvent.click(view.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(lastClaudeMcpUpdateInput()).toEqual(
        expect.objectContaining({
          removeEnvKeys: ["WEATHER_TOKEN"],
        }),
      );
    });
  });
});
