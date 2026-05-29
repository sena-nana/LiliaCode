import { describe, expect, it } from "vitest";
import {
  PLAN_APPROVAL_QUESTION_ID,
  buildPlanRevisionDenyMessage,
  buildPlanApprovalSpec,
  buildPlanPayload,
  extractPlanResult,
  isPlanApprovalAccepted,
  isReadonlyDeniedClaudeTool,
  normalizeClaudePermissionMode,
  readPlanRevisionRequest,
} from "../agent-runner/claudePlan.mjs";

describe("claudePlan helpers", () => {
  it("从 ExitPlanMode input 提取计划和允许提示", () => {
    const payload = buildPlanPayload({
      input: {
        plan: "## 计划\n- 先读代码\n- 再修改",
        allowedPrompts: [
          { tool: "Bash", prompt: "yarn test" },
          { tool: "Write", prompt: "" },
        ],
      },
      approved: null,
      executionPermission: "full",
    });

    expect(payload).toMatchObject({
      source: "ExitPlanMode",
      plan: "## 计划\n- 先读代码\n- 再修改",
      approved: null,
      executionPermission: "full",
      allowedPrompts: [{ tool: "Bash", prompt: "yarn test" }],
    });
  });

  it("从工具结果 JSON 提取 Claude 返回的计划元数据", () => {
    const result = extractPlanResult(JSON.stringify({
      plan: "确认后的计划",
      filePath: "plans/current.md",
      planWasEdited: true,
      awaitingLeaderApproval: true,
    }));

    expect(result).toMatchObject({
      plan: "确认后的计划",
      filePath: "plans/current.md",
      planWasEdited: true,
      awaitingLeaderApproval: true,
    });
  });

  it("确认规格只保留标题和动作，不内联计划正文或权限提示", () => {
    const spec = buildPlanApprovalSpec();

    expect(spec.title).toBe("确认 Claude 计划");
    expect(spec.intent).toBe("plan_approval");
    expect(spec.questions[0]?.id).toBe(PLAN_APPROVAL_QUESTION_ID);
    expect(spec.questions[0]?.question).toBe("");
    expect(spec.questions[0]?.confirmLabel).toBe("按计划执行");
  });

  it("解析计划确认的接受、取消和修改要求结果", () => {
    expect(isPlanApprovalAccepted({
      answers: { [PLAN_APPROVAL_QUESTION_ID]: { value: "yes" } },
    })).toBe(true);
    expect(isPlanApprovalAccepted({ cancelled: true })).toBe(false);
    expect(isPlanApprovalAccepted({
      answers: { [PLAN_APPROVAL_QUESTION_ID]: { value: "no" } },
    })).toBe(false);
    expect(readPlanRevisionRequest({
      cancelled: false,
      answers: {
        [PLAN_APPROVAL_QUESTION_ID]: {
          questionId: PLAN_APPROVAL_QUESTION_ID,
          value: "revision_request",
          notes: "先补充失败回滚方案",
        },
      },
    })).toBe("先补充失败回滚方案");
    expect(readPlanRevisionRequest({
      answers: { [PLAN_APPROVAL_QUESTION_ID]: { value: "no" } },
    })).toBe("");
    expect(buildPlanRevisionDenyMessage("先补充失败回滚方案")).toContain(
      "请根据这条修改要求调整计划",
    );
  });

  it("执行权限映射与只读写工具门禁保持分层", () => {
    expect(normalizeClaudePermissionMode("full")).toBe("bypassPermissions");
    expect(normalizeClaudePermissionMode("ask")).toBe("default");
    expect(normalizeClaudePermissionMode("readonly")).toBe("default");
    expect(isReadonlyDeniedClaudeTool("Write")).toBe(true);
    expect(isReadonlyDeniedClaudeTool("UnknownTool")).toBe(true);
    expect(isReadonlyDeniedClaudeTool("Read")).toBe(false);
    expect(isReadonlyDeniedClaudeTool("TodoWrite")).toBe(false);
  });
});
