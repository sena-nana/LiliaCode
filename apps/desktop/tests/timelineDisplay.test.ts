import { describe, expect, it } from "vitest";
import { deriveTimelineDisplay } from "@lilia/contracts";
import { normalizeClaudeTool } from "../../../packages/contracts/src/claudeTools.mjs";
import { timelineEventLabel, timelineInlinePreview } from "../src/components/chat/timelineDisplay";

function codeContent(
  details: ReturnType<typeof deriveTimelineDisplay>["details"],
  label: string,
): string {
  const detail = details?.find((item) => item.type === "code" && item.label === label);
  return detail?.type === "code" ? detail.content : "";
}

function listItems(details: ReturnType<typeof deriveTimelineDisplay>["details"]): string[] {
  return details?.flatMap((item) => item.type === "list" ? item.items.map((entry) => entry.text) : []) ?? [];
}

function markdownItems(details: ReturnType<typeof deriveTimelineDisplay>["details"]): string[] {
  return details?.flatMap((item) => item.type === "markdown" ? [item.content] : []) ?? [];
}

describe("timeline display derivation", () => {
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
    expect(markdownItems(display.details)).toContain("用户取消了提问。");
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
    expect(display.defaultExpanded).toBe(true);
    expect(markdownItems(display.details)).toContain("## 修改计划\n- 接线 runner\n- 补测试");
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
    expect(markdownItems(display.details)).toContain("## 当前计划\n- 先改 runner\n- 再补测试");
    expect(markdownItems(display.details)).toContain("把文档边界也写清楚");
    expect(display.group?.bucket).toBe("plan");
  });
});
