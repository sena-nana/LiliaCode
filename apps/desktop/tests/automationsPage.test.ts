import { fireEvent, render, waitFor, within } from "@testing-library/vue";
import { useVueFlow } from "@vue-flow/core";
import { describe, expect, it, vi } from "vitest";
import Automations from "../src/pages/Automations.vue";
import { clearMockAutomations, finishMockAutomationRun, mockInvoke } from "./tauriMock";

const vueFlowMock = vi.hoisted(() => ({
  useVueFlow: vi.fn(() => ({ fitView: vi.fn(), zoomIn: vi.fn(), zoomOut: vi.fn() })),
}));

vi.mock("@vue-flow/core", async () => {
  const { defineComponent, h } = await import("vue");
  const VueFlow = defineComponent({
    name: "VueFlow",
    props: {
      nodes: { type: Array, default: () => [] },
      edges: { type: Array, default: () => [] },
    },
    emits: ["node-click"],
    setup(props, { slots, emit }) {
      return () =>
        h(
          "div",
          { "data-testid": "automation-flow" },
          (props.nodes as Array<{ id: string; data: unknown }>).map((node) =>
            h(
              "button",
              {
                key: node.id,
                type: "button",
                onClick: () => emit("node-click", { node }),
              },
              slots["node-automation"]?.({ data: node.data }),
            ),
          ),
        );
    },
  });
  return {
    Handle: defineComponent({ name: "Handle", setup: () => () => h("span") }),
    Position: { Left: "left", Right: "right" },
    VueFlow,
    useVueFlow: vueFlowMock.useVueFlow,
  };
});

function lastInvokeInput(command: string) {
  return mockInvoke.mock.calls
    .filter(([calledCommand]) => calledCommand === command)
    .at(-1)?.[1];
}

function renderAutomations() {
  document.getElementById("automation-sidebar-host")?.closest("aside")?.remove();
  const sidebar = document.createElement("aside");
  sidebar.setAttribute("aria-label", "自动化列表");
  const host = document.createElement("div");
  host.id = "automation-sidebar-host";
  sidebar.appendChild(host);
  document.body.appendChild(sidebar);
  return render(Automations);
}

describe("Automations page", () => {
  it("展示全局自动化列表、节点画布和检查器", async () => {
    const zoomIn = vi.fn();
    const zoomOut = vi.fn();
    const fitView = vi.fn();
    vi.mocked(useVueFlow).mockReturnValue({ fitView, zoomIn, zoomOut } as never);
    const view = renderAutomations();

    await view.findByRole("complementary", { name: "自动化检查器" }, { timeout: 5000 });
    const list = view.getByRole("complementary", { name: "自动化列表" });
    const inspector = view.getByRole("complementary", { name: "自动化检查器" });

    await waitFor(() => {
      expect(within(list).getByRole("button", { name: /任务完成后复盘/ })).toBeInTheDocument();
    });
    expect(within(list).queryByRole("heading", { name: "自动化" })).not.toBeInTheDocument();
    expect(within(list).queryByRole("button", { name: "新建自动化" })).not.toBeInTheDocument();

    expect(view.getByRole("textbox", { name: "自动化名称" })).toHaveValue("任务完成后复盘");
    expect(within(view.getByTestId("automation-flow")).getByText("任务变化")).toBeInTheDocument();
    expect(within(view.getByTestId("automation-flow")).getByText("复盘 Agent")).toBeInTheDocument();
    expect(view.getByRole("group", { name: "画布控制" })).toBeInTheDocument();
    expect(view.getByRole("img", { name: "节点小地图" })).toBeInTheDocument();
    expect(view.getByRole("button", { name: "添加事件触发" })).toBeDisabled();
    expect(view.getByRole("button", { name: "添加人工确认" })).toBeEnabled();
    expect(view.getByRole("textbox", { name: "手动 Payload" })).toBeInTheDocument();
    await fireEvent.click(view.getByRole("button", { name: "放大画布" }));
    await fireEvent.click(view.getByRole("button", { name: "缩小画布" }));
    await fireEvent.click(view.getByRole("button", { name: "适应视图" }));
    expect(zoomIn).toHaveBeenCalled();
    expect(zoomOut).toHaveBeenCalled();
    expect(fitView).toHaveBeenCalledWith({ padding: 0.2 });
    expect(within(inspector).getByText("作用域")).toBeInTheDocument();
    expect(within(inspector).getByRole("button", { name: "Lilia" })).toBeInTheDocument();
    expect(within(inspector).getByRole("button", { name: "task_created" })).toBeInTheDocument();
    expect(within(inspector).getByRole("button", { name: "task_status_changed" })).toBeInTheDocument();
    expect(within(inspector).getByText("运行历史")).toBeInTheDocument();

    await fireEvent.click(view.getByRole("button", { name: "新建自动化" }));
    await waitFor(() => {
      expect(lastInvokeInput("automation_save_draft")?.input).toMatchObject({
        name: "未命名自动化",
      });
      expect(view.getByRole("textbox", { name: "自动化名称" })).toHaveValue("未命名自动化");
      expect(within(list).getByRole("button", { name: /未命名自动化/ })).toBeInTheDocument();
    });
    expect(view.getByRole("button", { name: "添加事件触发" })).toBeEnabled();

    const newWorkflowRow = within(list).getByRole("button", { name: /未命名自动化/ });
    await fireEvent.click(within(newWorkflowRow).getByRole("button", { name: "删除" }));
    expect(within(newWorkflowRow).getByRole("button", { name: "确认删除" })).toBeInTheDocument();
    await fireEvent.click(within(newWorkflowRow).getByRole("button", { name: "确认删除" }));
    await waitFor(() => {
      expect(lastInvokeInput("automation_delete_workflow")).toEqual({ id: "auto-2" });
      expect(within(list).queryByRole("button", { name: /未命名自动化/ })).not.toBeInTheDocument();
      expect(view.getByRole("textbox", { name: "自动化名称" })).toHaveValue("任务完成后复盘");
    });
  });

  it("没有自动化或删除最后一项后自动创建空白自动化", async () => {
    clearMockAutomations();
    const view = renderAutomations();
    const list = view.getByRole("complementary", { name: "自动化列表" });

    await waitFor(() => {
      expect(within(list).queryByText("没有自动化")).not.toBeInTheDocument();
      expect(within(list).getByRole("button", { name: /未命名自动化/ })).toBeInTheDocument();
      expect(view.getByRole("textbox", { name: "自动化名称" })).toHaveValue("未命名自动化");
      expect(lastInvokeInput("automation_save_draft")?.input).toMatchObject({
        name: "未命名自动化",
        nodes: [],
        edges: [],
      });
    });

    const row = within(list).getByRole("button", { name: /未命名自动化/ });
    await fireEvent.click(within(row).getByRole("button", { name: "删除" }));
    await fireEvent.click(within(row).getByRole("button", { name: "确认删除" }));

    await waitFor(() => {
      expect(lastInvokeInput("automation_delete_workflow")).toEqual({ id: "auto-1" });
      expect(lastInvokeInput("automation_save_draft")?.input).toMatchObject({
        name: "未命名自动化",
        nodes: [],
        edges: [],
      });
      expect(within(list).getByRole("button", { name: /未命名自动化/ })).toBeInTheDocument();
    });
  });

  it("暴露 Agent、工具和手动运行 Payload 配置", async () => {
    const view = renderAutomations();

    await waitFor(() => {
      expect(view.getByRole("textbox", { name: "自动化名称" })).toHaveValue("任务完成后复盘");
    });

    const flow = view.getByTestId("automation-flow");
    await fireEvent.click(within(flow).getByText("复盘 Agent"));
    const inspector = view.getByRole("complementary", { name: "自动化检查器" });

    await fireEvent.update(within(inspector).getByLabelText("模型"), "gpt-5.5");
    await fireEvent.update(within(inspector).getByLabelText("工作目录"), "C:\\Files\\workspace\\Lilia");

    await fireEvent.click(view.getByRole("button", { name: "添加工具" }));
    await fireEvent.change(within(inspector).getByLabelText("动作"), { target: { value: "send_guide" } });
    await fireEvent.update(within(inspector).getByLabelText("引导内容"), "请确认 ${trigger.taskId}");
    await fireEvent.change(within(inspector).getByLabelText("优先级"), { target: { value: "high" } });

    await fireEvent.click(view.getByRole("button", { name: "保存草稿" }));
    await waitFor(() => {
      expect(lastInvokeInput("automation_save_draft")?.input.nodes).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            id: "agent-1",
            config: expect.objectContaining({
              model: "gpt-5.5",
              projectCwd: "C:\\Files\\workspace\\Lilia",
            }),
          }),
          expect.objectContaining({
            kind: "tool",
            config: expect.objectContaining({
              action: "send_guide",
              text: "请确认 ${trigger.taskId}",
              priority: "high",
            }),
          }),
        ]),
      );
    });

    await fireEvent.click(view.getByRole("button", { name: "发布" }));
    await fireEvent.update(view.getByRole("textbox", { name: "手动 Payload" }), '{"reason":"smoke"}');
    await fireEvent.click(view.getByRole("button", { name: "手动运行" }));

    await waitFor(() => {
      expect(lastInvokeInput("automation_run_once")).toEqual({
        id: "auto-1",
        input: { payload: { reason: "smoke" } },
      });
    });
  });

  it("保存草稿、发布、启停并手动运行后显示运行历史和节点状态", async () => {
    const view = renderAutomations();

    await waitFor(() => {
      expect(view.getByRole("textbox", { name: "自动化名称" })).toHaveValue("任务完成后复盘");
    });

    await fireEvent.update(view.getByRole("textbox", { name: "自动化名称" }), "自动复盘");
    const inspector = view.getByRole("complementary", { name: "自动化检查器" });
    await fireEvent.click(within(inspector).getByRole("button", { name: "Lilia" }));
    await fireEvent.click(within(inspector).getByRole("button", { name: "running" }));
    await fireEvent.click(within(inspector).getByRole("button", { name: "claude" }));
    await fireEvent.click(view.getByRole("button", { name: "保存草稿" }));

    await waitFor(() => {
      expect(lastInvokeInput("automation_save_draft")?.input).toMatchObject({
        name: "自动复盘",
        scope: {
          projectIds: ["lilia"],
          taskStatuses: ["running"],
          backends: ["claude"],
        },
      });
    });

    await waitFor(() => {
      expect(view.getByRole("button", { name: /自动复盘/ })).toBeInTheDocument();
    });

    await fireEvent.click(view.getByRole("button", { name: "发布" }));
    await waitFor(() => {
      expect(lastInvokeInput("automation_publish")).toEqual({ id: "auto-1" });
    });

    await fireEvent.click(view.getByRole("button", { name: "启用" }));
    await waitFor(() => {
      expect(lastInvokeInput("automation_set_enabled")).toEqual({ id: "auto-1", enabled: true });
    });

    await fireEvent.click(view.getByRole("button", { name: "手动运行" }));
    await waitFor(() => {
      expect(lastInvokeInput("automation_run_once")).toEqual({ id: "auto-1", input: {} });
    });

    await waitFor(() => {
      expect(within(inspector).getByText("manual")).toBeInTheDocument();
      expect(within(inspector).getByText("项目 lilia · 任务 t-002 · 后端 claude · 事件 manual")).toBeInTheDocument();
      expect(within(inspector).getAllByText("running").length).toBeGreaterThan(0);
      expect(within(inspector).getAllByText("succeeded").length).toBeGreaterThan(0);
      expect(within(inspector).getByText("agent-1")).toBeInTheDocument();
      expect(within(inspector).getByText("输入")).toBeInTheDocument();
      expect(within(inspector).getByText("输出")).toBeInTheDocument();
      expect(within(inspector).getByText(/"trigger"/)).toBeInTheDocument();
    });

    const agentStateButton = within(inspector).getByText("agent-1").closest("button");
    expect(agentStateButton).not.toBeNull();
    await fireEvent.click(agentStateButton!);

    await waitFor(() => {
      expect(within(inspector).getByText(/"waitingAgent": true/)).toBeInTheDocument();
    });

    finishMockAutomationRun("run-1");
    await waitFor(() => {
      expect(within(inspector).getByText(/"completedByEvent": true/)).toBeInTheDocument();
      expect(within(inspector).getAllByText("succeeded").length).toBeGreaterThan(1);
    });
  });

  it("人工节点等待用户确认后可以继续运行", async () => {
    const view = renderAutomations();

    await waitFor(() => {
      expect(view.getByRole("textbox", { name: "自动化名称" })).toHaveValue("任务完成后复盘");
    });

    await fireEvent.click(view.getByRole("button", { name: "添加人工确认" }));
    await fireEvent.click(view.getByRole("button", { name: "保存草稿" }));
    await waitFor(() => {
      expect(lastInvokeInput("automation_save_draft")?.input.nodes).toEqual(
        expect.arrayContaining([expect.objectContaining({ kind: "human" })]),
      );
    });

    await fireEvent.click(view.getByRole("button", { name: "发布" }));
    await fireEvent.click(view.getByRole("button", { name: "手动运行" }));

    const inspector = view.getByRole("complementary", { name: "自动化检查器" });
    await waitFor(() => {
      expect(within(inspector).getAllByText("waiting_user").length).toBeGreaterThan(0);
    });

    let humanStateLabel: HTMLElement | null = null;
    await waitFor(() => {
      humanStateLabel = within(inspector).getByText(/^human-/);
      expect(humanStateLabel).toBeInTheDocument();
    });
    const humanStateButton = humanStateLabel.closest("button");
    expect(humanStateButton).not.toBeNull();
    await fireEvent.click(humanStateButton!);
    expect(within(inspector).getByRole("region", { name: "自动化等待确认" })).toBeInTheDocument();
    expect(within(inspector).getByText("人工节点")).toBeInTheDocument();
    await fireEvent.click(within(inspector).getByRole("button", { name: "继续" }));

    await waitFor(() => {
      expect(lastInvokeInput("automation_resume_run")).toMatchObject({
        runId: "run-1",
        input: {
          nodeId: expect.stringMatching(/^human-/),
          payload: { confirmed: true },
        },
      });
      expect(within(inspector).getAllByText("succeeded").length).toBeGreaterThan(0);
    });
  });
});
