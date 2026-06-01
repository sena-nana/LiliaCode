import { fireEvent, render } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import type { AgentTimelineEvent } from "@lilia/contracts";
import { deriveTimelineDisplay, isAgentTimelineToolWindowKind } from "@lilia/contracts";
import { normalizeClaudeTool } from "../../../packages/contracts/src/claudeTools.mjs";
import AgentTimeline from "../src/components/chat/AgentTimeline.vue";
import { timelineEventLabel, timelineInlinePreview } from "../src/components/chat/timelineDisplay";

function codeContent(
  details: ReturnType<typeof deriveTimelineDisplay>["details"],
  label: string,
): string {
  const detail = details?.find((item) => item.type === "code" && item.label === label);
  return detail?.type === "code" ? detail.content : "";
}

function fieldValue(
  details: ReturnType<typeof deriveTimelineDisplay>["details"],
  label: string,
): string {
  const fieldDetail = details?.find((item) => item.type === "fields");
  if (fieldDetail?.type !== "fields") return "";
  return fieldDetail.fields.find((field) => field.label === label)?.value ?? "";
}

function markdownContents(
  details: ReturnType<typeof deriveTimelineDisplay>["details"],
): string[] {
  return details
    ?.filter((item) => item.type === "markdown")
    .map((item) => item.type === "markdown" ? item.content : "") ?? [];
}

function listItems(
  details: ReturnType<typeof deriveTimelineDisplay>["details"],
): string[] {
  return details?.flatMap((item) =>
    item.type === "list" ? item.items.map((entry) => entry.text) : []
  ) ?? [];
}

describe("timeline display derivation", () => {
  it("识别所有会触发高优先级引导的工具窗口 kind", () => {
    for (const kind of [
      "tool",
      "command",
      "mcp",
      "search",
      "web_search",
      "web_fetch",
      "file_read",
      "file_change",
      "todo_list",
      "subagent",
    ]) {
      expect(isAgentTimelineToolWindowKind(kind)).toBe(true);
    }
    expect(isAgentTimelineToolWindowKind("message")).toBe(false);
    expect(isAgentTimelineToolWindowKind("plan")).toBe(false);
  });

  it("命令输出详情保留原始分行", () => {
    const display = deriveTimelineDisplay({
      kind: "command",
      status: "success",
      title: "yarn test",
      summary: "",
      payload: {
        command: "yarn test",
        output: "line one\nline two\nline three",
      },
    });

    expect(codeContent(display.details, "OUTPUT")).toBe("line one\nline two\nline three");
  });

  it("Claude AskUserQuestion 派生为提问事件并用问题文本做缩略", () => {
    const normalized = normalizeClaudeTool("AskUserQuestion", {
      questions: [
        {
          header: "方案",
          question: "选哪个方案？",
          options: [{ label: "方案 A" }, { label: "方案 B" }],
        },
        {
          header: "范围",
          question: "是否包含测试？",
          options: [{ label: "包含" }, { label: "不包含" }],
        },
      ],
    });

    const event = {
      kind: normalized.kind,
      status: "started" as const,
      title: "AskUserQuestion",
      summary: normalized.summary,
      payload: {
        toolName: "AskUserQuestion",
        ...normalized.payload,
      },
    };

    expect(normalized.kind).toBe("ask_user");
    expect(timelineEventLabel(event)).toBe("正在提问");
    expect(timelineInlinePreview(event)).toBe("方案 · 选哪个方案？ 等 2 个问题");
  });

  it("AskUserQuestion 完成后展开项显示用户选择内容", () => {
    const display = deriveTimelineDisplay({
      kind: "ask_user",
      status: "success",
      title: "AskUserQuestion",
      summary: "",
      payload: {
        toolName: "AskUserQuestion",
        questions: [
          {
            id: "q-1",
            header: "方案",
            question: "选哪个方案？",
            options: [{ label: "方案 A" }, { label: "方案 B" }],
          },
        ],
        output: JSON.stringify({
          answers: { "选哪个方案？": "方案 B" },
          annotations: { "选哪个方案？": { notes: "保留回滚入口" } },
          cancelled: false,
        }),
      },
    });

    expect(display.preview).toBe("方案 · 选哪个方案？");
    expect(listItems(display.details)).toContain(
      "方案 · 选哪个方案？：方案 B（备注：保留回滚入口）",
    );
    expect(codeContent(display.details, "OUTPUT")).toBe("");
  });

  it("AskUserQuestion 取消时保留已选择内容并显示取消态", () => {
    const event = {
      kind: "ask_user",
      status: "cancelled" as const,
      title: "AskUserQuestion",
      summary: "",
      payload: {
        toolName: "AskUserQuestion",
        questions: [
          {
            id: "q-1",
            header: "方案",
            question: "选哪个方案？",
            options: [{ label: "方案 A" }, { label: "方案 B" }],
          },
        ],
        output: JSON.stringify({
          answers: { "选哪个方案？": "方案 A" },
          cancelled: true,
        }),
      },
    };
    const display = deriveTimelineDisplay(event);

    expect(timelineEventLabel(event)).toBe("已取消提问");
    expect(listItems(display.details)).toContain("方案 · 选哪个方案？：方案 A");
    expect(markdownContents(display.details)).toContain("用户取消了提问。");
  });

  it("Claude ExitPlanMode 派生为待确认计划事件", () => {
    const normalized = normalizeClaudeTool("ExitPlanMode", {
      plan: "## 修改计划\n- 接线 runner\n- 补测试",
      allowedPrompts: [{ tool: "Bash", prompt: "yarn test" }],
    });

    const event = {
      kind: normalized.kind,
      status: "requires_action" as const,
      title: "ExitPlanMode",
      summary: "",
      payload: {
        toolName: "ExitPlanMode",
        ...normalized.payload,
        approved: null,
        executionPermission: "ask",
      },
    };
    const display = deriveTimelineDisplay(event);

    expect(normalized.kind).toBe("plan");
    expect(timelineEventLabel(event)).toBe("等待确认计划");
    expect(timelineInlinePreview(event)).toBe("## 修改计划 - 接线 runner - 补测试");
    expect(display.defaultExpanded).toBeUndefined();
    expect(markdownContents(display.details)).toContain("## 修改计划\n- 接线 runner\n- 补测试");
    expect(listItems(display.details)).toContain("Bash：yarn test");
  });

  it("计划确认和取消使用明确标签并保持 plan bucket", () => {
    const accepted = {
      kind: "plan",
      status: "success" as const,
      title: "ExitPlanMode",
      summary: "",
      payload: {
        plan: "按计划执行",
        approved: true,
        executionPermission: "full",
      },
    };
    const cancelled = {
      ...accepted,
      status: "cancelled" as const,
      payload: {
        ...accepted.payload,
        approved: false,
      },
    };

    expect(timelineEventLabel(accepted)).toBe("已确认计划");
    expect(deriveTimelineDisplay(accepted).group?.bucket).toBe("plan");
    expect(deriveTimelineDisplay(accepted).defaultExpanded).toBeUndefined();
    expect(timelineEventLabel(cancelled)).toBe("已取消计划");
  });

  it("计划修改要求显示明确标签并在详情保留原计划和要求", () => {
    const event = {
      kind: "plan",
      status: "cancelled" as const,
      title: "ExitPlanMode",
      summary: "",
      payload: {
        plan: "## 当前计划\n- 先改 runner\n- 再补测试",
        revisionRequest: "把文档边界也写清楚",
        approved: false,
        executionPermission: "ask",
      },
    };
    const display = deriveTimelineDisplay(event);

    expect(timelineEventLabel(event)).toBe("要求修改计划");
    expect(timelineInlinePreview(event)).toBe("修改要求：把文档边界也写清楚");
    expect(markdownContents(display.details)).toContain("## 当前计划\n- 先改 runner\n- 再补测试");
    expect(markdownContents(display.details)).toContain("把文档边界也写清楚");
    expect(display.group?.bucket).toBe("plan");
  });

  it("读取文件事件不把执行细节作为展开详情", () => {
    const display = deriveTimelineDisplay({
      kind: "file_read",
      status: "success",
      title: "Read",
      summary: "src/App.vue",
      payload: {
        path: "src/App.vue",
        offset: 10,
        limit: 20,
        output: "const app = createApp(App);",
      },
    });

    expect(display.preview).toBe("src/App.vue");
    expect(display.details).toBeUndefined();
  });

  it("查找文件和搜索内容事件不把执行细节作为展开详情", () => {
    const globDisplay = deriveTimelineDisplay({
      kind: "search",
      status: "success",
      title: "Glob",
      summary: "*.vue",
      payload: {
        subkind: "glob",
        query: "*.vue",
        path: "apps/desktop/src",
        output: "apps/desktop/src/App.vue",
      },
    });
    const grepDisplay = deriveTimelineDisplay({
      kind: "search",
      status: "success",
      title: "Grep",
      summary: "timelineCanExpand",
      payload: {
        subkind: "grep",
        query: "timelineCanExpand",
        path: "apps/desktop/src",
        glob: "*.ts",
        output: "apps/desktop/src/components/chat/timelineDisplay.ts",
      },
    });

    expect(globDisplay.preview).toBe("*.vue");
    expect(globDisplay.details).toBeUndefined();
    expect(grepDisplay.preview).toBe("timelineCanExpand");
    expect(grepDisplay.details).toBeUndefined();
  });

  it("命令和待办仍保留可展开详情", () => {
    const commandDisplay = deriveTimelineDisplay({
      kind: "command",
      status: "success",
      title: "yarn test",
      summary: "",
      payload: {
        command: "yarn test",
        output: "ok",
      },
    });
    const todoDisplay = deriveTimelineDisplay({
      kind: "todo_list",
      status: "success",
      title: "Todo",
      summary: "",
      payload: {
        items: [{ text: "验证 timeline", completed: true }],
      },
    });

    expect(commandDisplay.details?.length).toBeGreaterThan(0);
    expect(todoDisplay.details?.length).toBeGreaterThan(0);
  });

  it("短命令没有输出和排错字段时不提供展开详情", () => {
    const display = deriveTimelineDisplay({
      kind: "command",
      status: "success",
      title: "pwd",
      summary: "",
      payload: {
        command: "pwd",
      },
    });

    expect(display.preview).toBe("pwd");
    expect(display.details).toBeUndefined();
  });

  it("长命令即使没有输出也保留完整命令详情", () => {
    const command = `node scripts/build-report.js --workspace apps/desktop --filter timeline --output dist/reports/timeline-display.json --include-snapshots --include-contracts --max-old-space-size=4096 --verbose --dry-run`;
    const display = deriveTimelineDisplay({
      kind: "command",
      status: "success",
      title: command,
      summary: "",
      payload: { command },
    });

    expect(codeContent(display.details, "COMMAND")).toBe(command);
    expect(codeContent(display.details, "OUTPUT")).toBe("");
  });

  it("有输出的短命令保留完整命令和输出", () => {
    const display = deriveTimelineDisplay({
      kind: "command",
      status: "success",
      title: "pwd",
      summary: "",
      payload: {
        command: "pwd",
        output: "C:/Files/workspace/Lilia",
      },
    });

    expect(codeContent(display.details, "COMMAND")).toBe("pwd");
    expect(codeContent(display.details, "OUTPUT")).toBe("C:/Files/workspace/Lilia");
    expect(fieldValue(display.details, "exit")).toBe("");
  });

  it("成功单文件修改和网页类事件不展示重复详情", () => {
    const editDisplay = deriveTimelineDisplay({
      kind: "file_change",
      status: "success",
      title: "Edit",
      summary: "src/App.vue",
      payload: {
        subkind: "edit",
        path: "src/App.vue",
      },
    });
    const webSearchDisplay = deriveTimelineDisplay({
      kind: "search",
      status: "success",
      title: "WebSearch",
      summary: "Lilia timeline",
      payload: {
        subkind: "web",
        query: "Lilia timeline",
      },
    });
    const webFetchDisplay = deriveTimelineDisplay({
      kind: "web_fetch",
      status: "success",
      title: "WebFetch",
      summary: "https://example.com",
      payload: {
        url: "https://example.com",
      },
    });

    expect(editDisplay.details).toBeUndefined();
    expect(webSearchDisplay.details).toBeUndefined();
    expect(webFetchDisplay.details).toBeUndefined();
  });

  it("失败的文件修改和网页类事件保留错误详情", () => {
    const editDisplay = deriveTimelineDisplay({
      kind: "file_change",
      status: "error",
      title: "Edit",
      summary: "Permission denied",
      payload: {
        subkind: "edit",
        path: "src/App.vue",
        output: "Permission denied",
      },
    });
    const webFetchDisplay = deriveTimelineDisplay({
      kind: "web_fetch",
      status: "failed",
      title: "WebFetch",
      summary: "Network failed",
      payload: {
        url: "https://example.com",
        output: "Network failed",
      },
    });

    expect(codeContent(editDisplay.details, "ERROR / OUTPUT")).toBe("Permission denied");
    expect(codeContent(webFetchDisplay.details, "ERROR / OUTPUT")).toBe("Network failed");
  });

  it("MCP 成功态只在有输入或输出时展开", () => {
    const emptyDisplay = deriveTimelineDisplay({
      kind: "mcp",
      status: "success",
      title: "docs / search",
      summary: "docs / search",
      payload: {
        server: "docs",
        tool: "search",
      },
    });
    const outputDisplay = deriveTimelineDisplay({
      kind: "mcp",
      status: "success",
      title: "docs / search",
      summary: "docs / search",
      payload: {
        server: "docs",
        tool: "search",
        input: { query: "timeline" },
        output: "found docs",
      },
    });

    expect(emptyDisplay.details).toBeUndefined();
    expect(fieldValue(outputDisplay.details, "服务")).toBe("");
    expect(fieldValue(outputDisplay.details, "工具")).toBe("");
    expect(codeContent(outputDisplay.details, "INPUT")).toContain('"query": "timeline"');
    expect(codeContent(outputDisplay.details, "OUTPUT")).toBe("found docs");
  });

  it("兜底工具只展示输入输出，不重复工具名字段", () => {
    const display = deriveTimelineDisplay({
      kind: "tool",
      status: "success",
      title: "Index",
      summary: "索引完成",
      payload: {
        toolName: "Index",
        input: { scope: "workspace" },
        output: "indexed 42 files",
      },
    });

    expect(fieldValue(display.details, "工具")).toBe("");
    expect(codeContent(display.details, "INPUT")).toContain('"scope": "workspace"');
    expect(codeContent(display.details, "OUTPUT")).toBe("indexed 42 files");
  });

  it("子代理详情保留描述和结果但不展示内部 prompt", () => {
    const display = deriveTimelineDisplay({
      kind: "subagent",
      status: "success",
      title: "worker",
      summary: "worker",
      payload: {
        agentType: "worker",
        description: "检查 timeline 展示",
        prompt: "内部长提示词",
        result: "发现 2 个可优化点",
      },
    });

    expect(markdownContents(display.details)).toEqual([
      "检查 timeline 展示",
      "发现 2 个可优化点",
    ]);
  });

  it("失败的读取和搜索事件保留错误详情", () => {
    const readDisplay = deriveTimelineDisplay({
      kind: "file_read",
      status: "error",
      title: "Read",
      summary: "File not found",
      payload: {
        path: "missing.ts",
        output: "File not found",
      },
    });
    const searchDisplay = deriveTimelineDisplay({
      kind: "search",
      status: "failed",
      title: "Grep",
      summary: "grep failed",
      payload: {
        subkind: "grep",
        query: "needle",
        output: "grep failed",
      },
    });

    expect(codeContent(readDisplay.details, "ERROR / OUTPUT")).toBe("File not found");
    expect(codeContent(searchDisplay.details, "ERROR / OUTPUT")).toBe("grep failed");
  });

  it("项目内文件路径在展示态缩短为相对路径", () => {
    const display = deriveTimelineDisplay({
      kind: "file_read",
      status: "success",
      title: "Read",
      summary: "C:\\Files\\workspace\\Lilia\\apps\\desktop\\src\\App.vue",
      payload: {
        path: "C:\\Files\\workspace\\Lilia\\apps\\desktop\\src\\App.vue",
      },
      projectCwd: "c:\\files\\workspace\\lilia",
    });

    expect(display.object).toBe("apps/desktop/src/App.vue");
    expect(display.preview).toBe("apps/desktop/src/App.vue");
  });

  it("项目外路径、相似前缀路径和已有相对路径不被改写", () => {
    const outsideDisplay = deriveTimelineDisplay({
      kind: "file_read",
      status: "success",
      title: "Read",
      summary: "C:\\Files\\workspace\\Other\\src\\App.vue",
      payload: { path: "C:\\Files\\workspace\\Other\\src\\App.vue" },
      projectCwd: "c:\\files\\workspace\\lilia",
    });
    const prefixDisplay = deriveTimelineDisplay({
      kind: "file_read",
      status: "success",
      title: "Read",
      summary: "C:\\Files\\workspace\\LiliaNext\\src\\App.vue",
      payload: { path: "C:\\Files\\workspace\\LiliaNext\\src\\App.vue" },
      projectCwd: "C:\\Files\\workspace\\Lilia",
    });
    const relativeDisplay = deriveTimelineDisplay({
      kind: "file_read",
      status: "success",
      title: "Read",
      summary: "src/App.vue",
      payload: { path: "src/App.vue" },
      projectCwd: "C:\\Files\\workspace\\Lilia",
    });

    expect(outsideDisplay.preview).toBe("C:\\Files\\workspace\\Other\\src\\App.vue");
    expect(prefixDisplay.preview).toBe("C:\\Files\\workspace\\LiliaNext\\src\\App.vue");
    expect(relativeDisplay.preview).toBe("src/App.vue");
  });

  it("多文件修改详情列表显示项目相对路径", () => {
    const display = deriveTimelineDisplay({
      kind: "file_change",
      status: "success",
      title: "MultiEdit",
      summary: "",
      payload: {
        changes: [
          { kind: "edit", path: "C:/Files/workspace/Lilia/packages/contracts/src/index.ts" },
          { kind: "edit", path: "C:/Files/workspace/Lilia/apps/desktop/src/App.vue" },
        ],
      },
      projectCwd: "C:\\Files\\workspace\\Lilia",
    });

    expect(display.object).toBe("packages/contracts/src/index.ts");
    expect(display.preview).toBe("packages/contracts/src/index.ts");
    expect(listItems(display.details)).toEqual([
      "edit packages/contracts/src/index.ts",
      "edit apps/desktop/src/App.vue",
    ]);
  });

  it("path 和 cwd 字段详情显示项目相对路径", () => {
    const errorDisplay = deriveTimelineDisplay({
      kind: "error",
      status: "error",
      title: "错误",
      summary: "读取失败",
      payload: {
        path: "C:\\Files\\workspace\\Lilia\\packages\\contracts\\src\\index.ts",
      },
      projectCwd: "c:\\files\\workspace\\lilia",
    });
    const commandDisplay = deriveTimelineDisplay({
      kind: "command",
      status: "error",
      title: "pwd",
      summary: "failed",
      payload: {
        command: "pwd",
        cwd: "C:\\Files\\workspace\\Lilia",
        exitCode: 1,
        stderr: "failed",
      },
      projectCwd: "c:\\files\\workspace\\lilia",
    });

    expect(fieldValue(errorDisplay.details, "path")).toBe("packages/contracts/src/index.ts");
    expect(fieldValue(commandDisplay.details, "cwd")).toBe(".");
  });

  it("命令代码块不改写项目内路径", () => {
    const command = "Get-Content -LiteralPath 'C:\\Files\\workspace\\Lilia\\package.json'";
    const display = deriveTimelineDisplay({
      kind: "command",
      status: "success",
      title: command,
      summary: "",
      payload: {
        command,
        output: "ok",
      },
      projectCwd: "C:\\Files\\workspace\\Lilia",
    });

    expect(codeContent(display.details, "COMMAND")).toBe(command);
  });
});

function timelineEvent(
  patch: Partial<AgentTimelineEvent> & Pick<AgentTimelineEvent, "id" | "kind" | "payload">,
): AgentTimelineEvent {
  return {
    id: patch.id,
    taskId: "task-1",
    turnId: patch.turnId ?? null,
    backend: "claude",
    kind: patch.kind,
    status: patch.status ?? "success",
    title: patch.title ?? patch.kind,
    summary: patch.summary ?? "",
    payload: patch.payload,
    createdAt: patch.createdAt ?? 1,
    updatedAt: patch.updatedAt ?? patch.createdAt ?? 1,
    turnSeq: patch.turnSeq ?? 1,
    intraTurnOrder: patch.intraTurnOrder ?? 1,
  };
}

describe("timeline event expansion", () => {
  it("为滚动地图保留用户消息、分组事件和错误事件的定位锚点", () => {
    const view = render(AgentTimeline, {
      props: {
        events: [
          timelineEvent({
            id: "user-1",
            kind: "message",
            payload: { role: "user", content: "开始" },
            intraTurnOrder: 1,
          }),
          timelineEvent({
            id: "plan-1",
            kind: "plan",
            payload: { plan: "第一步" },
            createdAt: 2,
            updatedAt: 2,
            intraTurnOrder: 2,
          }),
          timelineEvent({
            id: "plan-2",
            kind: "plan",
            payload: { plan: "第二步" },
            createdAt: 3,
            updatedAt: 3,
            intraTurnOrder: 3,
          }),
          timelineEvent({
            id: "error-1",
            kind: "command",
            status: "failed",
            payload: { command: "yarn test", error: "boom" },
            createdAt: 4,
            updatedAt: 4,
            intraTurnOrder: 4,
          }),
        ],
      },
    });

    expect(view.container.querySelector('[data-scroll-anchor-id="user-1"]'))
      .toHaveClass("agent-timeline__message-row");
    expect(view.container.querySelector('[data-scroll-anchor-ids~="plan-1"]'))
      .toHaveAttribute("data-scroll-anchor-ids", "plan-1 plan-2");
    expect(view.container.querySelector('[data-scroll-anchor-id="error-1"]'))
      .toHaveClass("agent-timeline__item");
  });

  it("历史待确认计划默认折叠", () => {
    const view = render(AgentTimeline, {
      props: {
        events: [
          timelineEvent({
            id: "plan-pending-history",
            kind: "plan",
            status: "requires_action",
            title: "ExitPlanMode",
            turnId: "turn-history",
            payload: {
              plan: "## 加载的计划\n- 保持折叠",
              approved: null,
              executionPermission: "ask",
            },
          }),
        ],
      },
    });

    const card = view.container.querySelector(".timeline-card--plan");
    const toggle = view.getByRole("button", { name: /等待确认计划/ });

    expect(card).toHaveClass("is-collapsed");
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(view.getByText("## 加载的计划 - 保持折叠")).toBeInTheDocument();
    expect(view.queryByText("加载的计划")).not.toBeInTheDocument();
  });

  it("活跃计划确认默认展开并使用专用计划卡片", () => {
    const view = render(AgentTimeline, {
      props: {
        activePlanApprovalTurnId: "turn-plan",
        events: [
          timelineEvent({
            id: "plan-pending",
            kind: "plan",
            status: "requires_action",
            title: "ExitPlanMode",
            turnId: "turn-plan",
            payload: {
              plan: "## 修改计划\n- 接线 runner\n- 补测试",
              allowedPrompts: [{ tool: "Bash", prompt: "yarn test" }],
              approved: null,
              executionPermission: "ask",
            },
          }),
        ],
      },
    });

    const card = view.container.querySelector(".timeline-card--plan");
    const toggle = view.getByRole("button", { name: /等待确认计划/ });
    const badge = view.container.querySelector(".timeline-plan-card__badge");

    expect(card).toHaveClass("is-expanded");
    expect(badge).toHaveClass("timeline-plan-card__badge--pending");
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(view.queryByText("## 修改计划 - 接线 runner - 补测试"))
      .not.toBeInTheDocument();
    expect(view.getByText("修改计划")).toBeInTheDocument();
    expect(view.getByText("可能调用")).toBeInTheDocument();
    expect(view.getByText("Bash：yarn test")).toBeInTheDocument();
  });

  it("计划正文保留 markdown 标题结构但不生成内层滚动地图", () => {
    const view = render(AgentTimeline, {
      props: {
        activePlanApprovalTurnId: "turn-outline",
        events: [
          timelineEvent({
            id: "plan-outline",
            kind: "plan",
            status: "requires_action",
            title: "ExitPlanMode",
            turnId: "turn-outline",
            payload: {
              plan: [
                "# 阶段一",
                "- 梳理边界",
                "",
                "## 阶段二",
                "- 实现交互",
                "",
                "### 阶段三",
                "- 收尾验证",
              ].join("\n"),
              approved: null,
              executionPermission: "ask",
            },
          }),
        ],
      },
    });
    const body = view.container.querySelector(".timeline-plan-card__body");
    const headings = view.container.querySelectorAll(
      ".timeline-plan-card__markdown--plan .markdown-block__heading",
    );

    expect(body).toBeInstanceOf(HTMLElement);
    expect(headings).toHaveLength(3);
    expect(view.container.querySelector(".timeline-plan-card__scroll-map")).toBeNull();
    expect(view.queryByRole("button", { name: /跳到计划标题/ })).not.toBeInTheDocument();
  });

  it("处理后的计划默认折叠并可再次展开", async () => {
    const acceptedView = render(AgentTimeline, {
      props: {
        events: [
          timelineEvent({
            id: "plan-accepted",
            kind: "plan",
            status: "success",
            title: "ExitPlanMode",
            payload: {
              plan: "## 已确认计划\n- 执行改动",
              approved: true,
              executionPermission: "full",
            },
          }),
        ],
      },
    });
    expect(acceptedView.container.querySelector(".timeline-card--plan"))
      .toHaveClass("is-collapsed");
    expect(acceptedView.container.querySelector(".timeline-plan-card__badge"))
      .toHaveClass("timeline-plan-card__badge--approved");
    expect(acceptedView.getByRole("button", { name: /已确认计划/ }))
      .toHaveAttribute("aria-expanded", "false");
    acceptedView.unmount();

    const cancelledView = render(AgentTimeline, {
      props: {
        events: [
          timelineEvent({
            id: "plan-cancelled",
            kind: "plan",
            status: "cancelled",
            title: "ExitPlanMode",
            payload: {
              plan: "## 已取消计划\n- 暂不执行",
              approved: false,
              executionPermission: "ask",
            },
          }),
        ],
      },
    });
    expect(cancelledView.container.querySelector(".timeline-card--plan"))
      .toHaveClass("is-collapsed");
    expect(cancelledView.container.querySelector(".timeline-plan-card__badge"))
      .toHaveClass("timeline-plan-card__badge--rejected");
    expect(cancelledView.getByRole("button", { name: /已取消计划/ }))
      .toHaveAttribute("aria-expanded", "false");
    cancelledView.unmount();

    const revisionView = render(AgentTimeline, {
      props: {
        events: [
          timelineEvent({
            id: "plan-revision",
            kind: "plan",
            status: "cancelled",
            title: "ExitPlanMode",
            payload: {
              plan: "## 当前计划\n- 先改 runner\n- 再补测试",
              revisionRequest: "把文档边界也写清楚",
              approved: false,
              executionPermission: "ask",
            },
          }),
        ],
      },
    });
    const revisionToggle = revisionView.getByRole("button", { name: /要求修改计划/ });

    expect(revisionView.container.querySelector(".timeline-card--plan"))
      .toHaveClass("is-collapsed");
    expect(revisionView.container.querySelector(".timeline-plan-card__badge"))
      .toHaveClass("timeline-plan-card__badge--revision");
    expect(revisionToggle).toHaveAttribute("aria-expanded", "false");
    expect(revisionView.getByText("修改要求：把文档边界也写清楚")).toBeInTheDocument();
    expect(revisionView.queryByText("先改 runner")).not.toBeInTheDocument();

    await fireEvent.click(revisionToggle);

    expect(revisionView.container.querySelector(".timeline-card--plan"))
      .toHaveClass("is-expanded");
    expect(revisionToggle).toHaveAttribute("aria-expanded", "true");
    expect(revisionView.getByLabelText("修改要求")).toBeInTheDocument();
    expect(revisionView.getByText("先改 runner")).toBeInTheDocument();
    expect(revisionView.getByText("把文档边界也写清楚")).toBeInTheDocument();

    await fireEvent.click(revisionToggle);

    expect(revisionView.container.querySelector(".timeline-card--plan"))
      .toHaveClass("is-collapsed");
    expect(revisionToggle).toHaveAttribute("aria-expanded", "false");
  });

  it("无详情的读取和搜索事件不提供展开入口", async () => {
    const view = render(AgentTimeline, {
      props: {
        events: [
          timelineEvent({
            id: "read-1",
            kind: "file_read",
            title: "Read",
            summary: "src/App.vue",
            payload: {
              path: "src/App.vue",
              offset: 10,
              limit: 20,
              output: "const app = createApp(App);",
            },
          }),
          timelineEvent({
            id: "search-1",
            kind: "search",
            title: "Glob",
            summary: "*.vue",
            payload: {
              subkind: "glob",
              query: "*.vue",
              path: "apps/desktop/src",
              output: "apps/desktop/src/App.vue",
            },
            createdAt: 2,
            updatedAt: 2,
            intraTurnOrder: 2,
          }),
        ],
      },
    });

    expect(view.container.querySelector(".agent-timeline__chevron")).not.toBeInTheDocument();

    await fireEvent.click(view.getByRole("button", { name: "已读取 src/App.vue" }));
    await fireEvent.click(view.getByRole("button", { name: "已查找文件 *.vue" }));

    expect(view.container.querySelector(".agent-timeline__content")).not.toBeInTheDocument();
  });

  it("有详情事件仍可展开", async () => {
    const view = render(AgentTimeline, {
      props: {
        events: [
          timelineEvent({
            id: "command-1",
            kind: "command",
            title: "yarn test",
            summary: "",
            payload: {
              command: "yarn test",
              output: "ok",
            },
          }),
        ],
      },
    });

    expect(view.container.querySelector(".agent-timeline__chevron")).toBeInTheDocument();

    await fireEvent.click(view.getByRole("button", { name: "已运行 yarn test" }));

    expect(view.container.querySelector(".agent-timeline__content")).toBeInTheDocument();
    expect(view.getByText("COMMAND")).toBeInTheDocument();
  });

  it("长命令可展开查看完整命令，短命令无详情时不可展开", async () => {
    const longCommand = `node scripts/build-report.js --workspace apps/desktop --filter timeline --output dist/reports/timeline-display.json --include-snapshots --include-contracts --max-old-space-size=4096 --verbose --dry-run`;
    const shortView = render(AgentTimeline, {
      props: {
        events: [
          timelineEvent({
            id: "short-command",
            kind: "command",
            title: "pwd",
            summary: "",
            payload: { command: "pwd" },
          }),
        ],
      },
    });
    expect(shortView.getByRole("button", { name: "已运行 pwd" }))
      .toHaveAttribute("aria-expanded", "false");
    expect(shortView.container.querySelector(".agent-timeline__chevron")).not.toBeInTheDocument();

    await fireEvent.click(shortView.getByRole("button", { name: "已运行 pwd" }));
    expect(shortView.container.querySelector(".agent-timeline__content")).not.toBeInTheDocument();
    shortView.unmount();

    const longView = render(AgentTimeline, {
      props: {
        events: [
          timelineEvent({
            id: "long-command",
            kind: "command",
            title: longCommand,
            summary: "",
            payload: { command: longCommand },
            createdAt: 2,
            updatedAt: 2,
            intraTurnOrder: 2,
          }),
        ],
      },
    });

    expect(longView.getByRole("button", { name: `已运行 ${longCommand}` }))
      .toHaveAttribute("aria-expanded", "false");
    expect(longView.container.querySelector(".agent-timeline__chevron")).toBeInTheDocument();

    await fireEvent.click(longView.getByRole("button", { name: `已运行 ${longCommand}` }));
    expect(longView.getByText("COMMAND")).toBeInTheDocument();
    expect(longView.getByText(longCommand)).toBeInTheDocument();
  });

  it("传入项目路径后折叠态显示相对路径", async () => {
    const view = render(AgentTimeline, {
      props: {
        projectCwd: "C:\\Files\\workspace\\Lilia",
        events: [
          timelineEvent({
            id: "read-absolute",
            kind: "file_read",
            title: "Read",
            summary: "C:\\Files\\workspace\\Lilia\\apps\\desktop\\src\\App.vue",
            payload: {
              path: "C:\\Files\\workspace\\Lilia\\apps\\desktop\\src\\App.vue",
            },
          }),
        ],
      },
    });

    expect(view.getByRole("button", { name: "已读取 apps/desktop/src/App.vue" }))
      .toBeInTheDocument();
    expect(view.getByText("apps/desktop/src/App.vue")).toBeInTheDocument();
    expect(view.queryByText("C:\\Files\\workspace\\Lilia\\apps\\desktop\\src\\App.vue"))
      .not.toBeInTheDocument();
  });
});
