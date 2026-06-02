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

  it("Codex 新会话和续聊都会跳过 Git 仓库检查", () => {
    expect(runnerSource).toContain("const threadOptions = {");
    expect(runnerSource).toContain("skipGitRepoCheck: true");
    expect(runnerSource).toContain("codex.resumeThread(resumeSessionId, threadOptions)");
    expect(runnerSource).toContain("codex.startThread(threadOptions)");
  });

  it("Codex 使用通用 assistant message timeline helper", () => {
    expect(runnerSource).toContain("function emitAssistantMessageTimeline");
    expect(runnerSource).toMatch(/function emitAssistantMessageTimeline\(text, status, backend = "assistant"\)/);
    expect(runnerSource).toMatch(/sourceId:\s*`\$\{backend\}:text:message`/);
    expect(runnerSource).toContain('emit: (text) => emitAssistantMessageTimeline(text, "running", "codex")');
    expect(runnerSource).toContain('emitAssistantMessageTimeline(finalText, "success", "codex")');
  });

  it("Codex MCP 配置发现事件带 config 子分类，真实 MCP 调用仍由 mcp_tool_call 映射", () => {
    expect(runnerSource).toMatch(/case "mcp_tool_call":\s*return \{ kind: "mcp" \};/);
    expect(runnerSource).toContain('title: "Codex MCP config"');
    expect(runnerSource).toMatch(/payload:\s*\{[\s\S]*?subkind:\s*"config"[\s\S]*?source:\s*"config\.toml"/);
  });
});
