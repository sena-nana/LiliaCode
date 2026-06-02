import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const testsDir = dirname(fileURLToPath(import.meta.url));
const runnerSource = readFileSync(join(testsDir, "..", "agent-runner.mjs"), "utf8");

describe("agent-runner Claude stream", () => {
  it("Claude AskUserQuestion 走 Lilia AskUser 请求/响应通道", () => {
    expect(runnerSource).toContain("createSdkMcpServer");
    expect(runnerSource).toContain("requestAskUser");
    expect(runnerSource).toContain("ask_user_request");
    expect(runnerSource).toContain("ask_user_response");
    expect(runnerSource).toContain("AskUserQuestion");
    expect(runnerSource).toContain("mcp__lilia__ask_user_question");
    expect(runnerSource).toMatch(/toolAliases:\s*\{\s*AskUserQuestion:/);
  });


  it("Claude plan mode 先进入计划，确认后恢复原执行权限", () => {
    expect(runnerSource).toContain("normalizeClaudePermissionMode");
    expect(runnerSource).toContain("mapClaudeInitialPermission(permission, planMode)");
    expect(runnerSource).toMatch(
      /permissionMode:\s*planMode\s*\?\s*"plan"\s*:\s*execution\.permissionMode/,
    );
    expect(runnerSource).toContain("handleClaudePlanPermission");
    expect(runnerSource).toContain("buildPlanApprovalSpec");
    expect(runnerSource).toContain('status: "requires_action"');
    expect(runnerSource).toContain("scheduleClaudePermissionModeRestore(ctx, mode)");
    expect(runnerSource).toContain('updatedPermissions: [{ type: "setMode", mode, destination: "session" }]');
    expect(runnerSource).toContain("singleClaudePromptStream");
  });


  it("本轮附件会作为路径上下文注入 Claude/Codex prompt", () => {
    expect(runnerSource).toContain("function buildPromptWithAttachments");
    expect(runnerSource).toContain("用户随本轮消息附加的本地路径");
    expect(runnerSource).toContain("不要假设已经读取了内容");
    expect(runnerSource).toMatch(/cmd\.prompt\s*=\s*buildPromptWithAttachments\(/);
    expect(runnerSource).toMatch(/attachments[\s\S]*?path/);
  });

  it("backend 为 codex 时只进入 Codex runner 分支", () => {
    expect(runnerSource).toContain('const backend = cmd.backend === "codex" ? "codex" : "claude"');
    expect(runnerSource).toMatch(/if\s*\(backend === "codex"\)\s*\{\s*await runCodex\(cmd\);/);
    expect(runnerSource).toMatch(/}\s*else\s*\{\s*await runClaude\(cmd\);/);
  });
});
