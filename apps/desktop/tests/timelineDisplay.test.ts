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
});
