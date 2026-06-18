import { fireEvent, render, waitFor, within } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import Plugins from "../src/pages/Plugins.vue";
import { mockInvoke } from "./tauriMock";

function lastCodexMcpCreateInput() {
  const call = mockInvoke.mock.calls
    .filter(([command, args]) => command === "plugins_create_mcp_server" && args?.backend === "codex")
    .at(-1);
  return call?.[1]?.input;
}

describe("Plugins page", () => {
  it("混合显示全局和项目 Skill，并把新建入口放在顶部", async () => {
    const view = render(Plugins);
    const list = view.getByRole("region", { name: "检索结果" });
    const detail = view.getByRole("region", { name: "插件和技能详情" });

    await waitFor(() => {
      expect(within(list).getByRole("button", { name: /mock-skill/ })).toBeInTheDocument();
    });
    expect(within(list).getByRole("button", { name: /project-skill.*Lilia/ })).toBeInTheDocument();
    expect(view.queryByRole("radiogroup", { name: "Skill scope" })).not.toBeInTheDocument();
    expect(view.getByRole("button", { name: "新建 Skill" })).toBeInTheDocument();

    await fireEvent.click(within(list).getByRole("button", { name: /project-skill/ }));
    expect(within(detail).getByText(/项目 · 已启用 · Lilia/)).toBeInTheDocument();
    expect(within(detail).getByText("Lilia")).toBeInTheDocument();
  });

  it("展示并管理 Claude MCP server", async () => {
    const view = render(Plugins);

    await fireEvent.click(view.getByRole("tab", { name: /Claude MCP/ }));
    const list = view.getByRole("region", { name: "检索结果" });
    const detail = view.getByRole("region", { name: "插件和技能详情" });

    await waitFor(() => {
      expect(within(list).getByRole("button", { name: /weather/ })).toBeInTheDocument();
    });
    expect(within(detail).getByText("node weather-mcp.js")).toBeInTheDocument();
    expect(within(detail).getByText("WEATHER_TOKEN")).toBeInTheDocument();

    await fireEvent.update(view.getByRole("searchbox", { name: "搜索插件和技能" }), "weather");
    expect(within(list).getByRole("button", { name: /weather/ })).toBeInTheDocument();
    await fireEvent.update(view.getByRole("searchbox", { name: "搜索插件和技能" }), "missing");
    expect(within(list).getByText("没有匹配项")).toBeInTheDocument();
    await fireEvent.update(view.getByRole("searchbox", { name: "搜索插件和技能" }), "");

    await fireEvent.click(view.getByRole("button", { name: "停用" }));
    await waitFor(() => {
      expect(within(detail).getByText(/已停用/)).toBeInTheDocument();
    });
    expect(
      mockInvoke.mock.calls.some(
        ([command, args]) =>
          command === "plugins_set_mcp_server_enabled" && args?.backend === "claude",
      ),
    ).toBe(true);

    await fireEvent.click(view.getByRole("button", { name: /新增 MCP/ }));
    await waitFor(() => {
      expect(view.getByPlaceholderText("weather-mcp")).toBeInTheDocument();
    });
    await fireEvent.update(view.getByPlaceholderText("weather-mcp"), "linear");
    await fireEvent.update(view.getByPlaceholderText("node"), "uvx");
    await fireEvent.update(view.getByPlaceholderText("每行一个参数"), "linear-mcp");
    await fireEvent.click(view.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(within(list).getByRole("button", { name: /linear/ })).toBeInTheDocument();
    });
    expect(
      mockInvoke.mock.calls.some(
        ([command, args]) =>
          command === "plugins_create_mcp_server" && args?.backend === "claude",
      ),
    ).toBe(true);
    await fireEvent.click(within(list).getByRole("button", { name: /linear/ }));
    expect(within(detail).getByText("uvx linear-mcp")).toBeInTheDocument();
  });

  it("管理 Codex stdio MCP 并保持 HTTP server 只读", async () => {
    const view = render(Plugins);

    await fireEvent.click(view.getByRole("tab", { name: /Codex MCP/ }));
    const list = view.getByRole("region", { name: "检索结果" });
    const detail = view.getByRole("region", { name: "插件和技能详情" });

    await waitFor(() => {
      expect(within(list).getByRole("button", { name: /mock-mcp/ })).toBeInTheDocument();
    });
    expect(within(list).getByRole("button", { name: /remote-mcp/ })).toBeInTheDocument();
    expect(within(detail).getByText("MOCK_TOKEN")).toBeInTheDocument();

    await fireEvent.click(within(list).getByRole("button", { name: /remote-mcp/ }));
    expect(within(detail).getByText("HTTP")).toBeInTheDocument();
    expect(within(detail).getByText("只读")).toBeInTheDocument();
    expect(view.queryByRole("button", { name: "停用" })).not.toBeInTheDocument();
    expect(view.queryByRole("button", { name: "编辑" })).not.toBeInTheDocument();
    expect(view.queryByRole("button", { name: "删除" })).not.toBeInTheDocument();

    await fireEvent.click(within(list).getByRole("button", { name: /mock-mcp/ }));

    await fireEvent.click(view.getByRole("button", { name: "停用" }));
    await waitFor(() => {
      expect(within(detail).getByText(/已停用/)).toBeInTheDocument();
    });
    expect(
      mockInvoke.mock.calls.some(
        ([command, args]) =>
          command === "plugins_set_mcp_server_enabled" && args?.backend === "codex",
      ),
    ).toBe(true);

    await fireEvent.click(view.getByRole("button", { name: /新增 MCP/ }));
    await waitFor(() => {
      expect(view.getByPlaceholderText("weather-mcp")).toBeInTheDocument();
    });
    await fireEvent.update(view.getByPlaceholderText("weather-mcp"), "codex-linear");
    await fireEvent.update(view.getByPlaceholderText("node"), "uvx");
    await fireEvent.update(view.getByPlaceholderText("每行一个参数"), "linear-mcp");
    await fireEvent.update(view.getByPlaceholderText("KEY"), "LINEAR_API_KEY");
    await fireEvent.update(view.getByPlaceholderText("value"), "secret");
    await fireEvent.click(view.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(within(list).getByRole("button", { name: /codex-linear/ })).toBeInTheDocument();
    });
    expect(lastCodexMcpCreateInput()).toMatchObject({
      name: "codex-linear",
      command: "uvx",
      args: ["linear-mcp"],
      env: { LINEAR_API_KEY: "secret" },
    });
    await fireEvent.click(within(list).getByRole("button", { name: /codex-linear/ }));
    expect(within(detail).getByText("uvx linear-mcp")).toBeInTheDocument();
  });

  it("展示 Claude Hooks 并在编辑结构化字段后保留高级 JSON", async () => {
    const view = render(Plugins);

    await fireEvent.click(view.getByRole("tab", { name: /Claude Hooks/ }));
    const list = view.getByRole("region", { name: "检索结果" });
    const detail = view.getByRole("region", { name: "插件和技能详情" });

    await waitFor(() => {
      expect(within(list).getByRole("button", { name: /Claude Project Hooks/ })).toBeInTheDocument();
    });
    await fireEvent.click(within(list).getByRole("button", { name: /Claude Project Hooks/ }));
    await waitFor(() => {
      expect(within(detail).getAllByText(/MODE/).length).toBeGreaterThan(0);
    });

    await fireEvent.click(view.getByRole("button", { name: "编辑" }));
    await waitFor(() => {
      expect(view.getByDisplayValue("node project-hook.js")).toBeInTheDocument();
    });
    const commandInput = view.getByDisplayValue("node project-hook.js");
    await fireEvent.update(commandInput, "node changed-hook.js");
    await fireEvent.click(view.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(within(detail).getByText("node changed-hook.js")).toBeInTheDocument();
    });
    expect(within(detail).getAllByText(/MODE/).length).toBeGreaterThan(0);
    expect(
      mockInvoke.mock.calls.some(
        ([command, args]) =>
          command === "plugins_update_hook_source" &&
          Array.isArray(args?.input?.handlers) &&
          args.input.handlers[0]?.advancedJson?.includes?.("MODE"),
      ),
    ).toBe(true);
  });

  it("支持创建缺失的 Hooks 来源，并对 Codex 只读来源展示限制", async () => {
    const view = render(Plugins);

    await fireEvent.click(view.getByRole("tab", { name: /Claude Hooks/ }));
    const list = view.getByRole("region", { name: "检索结果" });
    const detail = view.getByRole("region", { name: "插件和技能详情" });

    await waitFor(() => {
      expect(within(list).getByRole("button", { name: /Claude Local Hooks/ })).toBeInTheDocument();
    });
    await fireEvent.click(within(list).getByRole("button", { name: /Claude Local Hooks/ }));
    expect(within(detail).getByText("未创建")).toBeInTheDocument();
    await fireEvent.click(view.getByRole("button", { name: "创建来源" }));

    await waitFor(() => {
      expect(within(detail).getByText("空来源")).toBeInTheDocument();
    });
    expect(
      mockInvoke.mock.calls.some(
        ([command, args]) =>
          command === "plugins_create_hook_source" &&
          args?.backend === "claude" &&
          args?.scope === "local",
      ),
    ).toBe(true);

    await fireEvent.click(view.getByRole("tab", { name: /Codex Hooks/ }));
    await waitFor(() => {
      expect(within(list).getByRole("button", { name: /Codex User Config Hooks/ })).toBeInTheDocument();
    });
    await fireEvent.click(within(list).getByRole("button", { name: /Codex User Config Hooks/ }));
    await waitFor(() => {
      expect(within(detail).getByText(/Lilia 不会自动重写 config.toml/)).toBeInTheDocument();
    });
    expect(view.queryByRole("button", { name: "编辑" })).not.toBeInTheDocument();
    expect(view.queryByRole("button", { name: "删除" })).not.toBeInTheDocument();
  });
});
