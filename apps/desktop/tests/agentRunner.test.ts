import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const testsDir = dirname(fileURLToPath(import.meta.url));
const runnerSource = readFileSync(join(testsDir, "..", "agent-runner.mjs"), "utf8");
const packageManifest = readFileSync(join(testsDir, "..", "package.json"), "utf8");

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

  it("Codex 优先通过 app-server stdio 建立可交互通道", () => {
    expect(runnerSource).toContain("createCodexAppServer");
    expect(runnerSource).toContain("process.env.LILIA_CODEX_CLI_PATH");
    expect(runnerSource).toContain('spawnCodexCandidateSync(injected, ["--version"], { stdio: "ignore" })');
    expect(runnerSource).not.toContain('["codex.cmd", "codex.exe", "codex.bat", "codex"]');
    expect(runnerSource).toContain("function spawnCodexAppServer");
    expect(runnerSource).toContain('windowsCommandLine(binary, ["app-server"])');
    expect(runnerSource).toContain("spawnCodexAppServer(binary, {");
    expect(runnerSource).toContain('server.request("initialize"');
    expect(runnerSource).toContain('server.notify("initialized"');
    expect(runnerSource).toContain('server.request("thread/start"');
    expect(runnerSource).toContain('server.request("thread/resume"');
    expect(runnerSource).toContain('server.request("turn/start"');
  });

  it("Codex app-server 不再保留旧 Codex 通道或能力提示事件", () => {
    expect(runnerSource).toContain("runCodexAppServer(cmd, runtimeExtensions)");
    expect(runnerSource).not.toContain("emitCodexInteractionCapabilityTimeline");
    expect(runnerSource).not.toContain("Codex interaction capability");
    expect(runnerSource).not.toContain("codex:interaction:capability");
    expect(runnerSource).not.toContain("AskUser 工具注入不可用");
    expect(runnerSource).not.toContain('import("@openai/codex-sdk")');
    expect(runnerSource).not.toContain("runStreamed");
    expect(runnerSource).not.toContain("codex.resumeThread");
    expect(runnerSource).not.toContain("codex.startThread");
    expect(runnerSource).not.toContain("codexVendorBinaryPath");
    expect(runnerSource).not.toContain("newConversation");
    expect(runnerSource).not.toContain("resumeConversation");
    expect(runnerSource).not.toContain("sendUserMessage");
    expect(runnerSource).not.toContain("addConversationListener");
    expect(runnerSource).not.toContain("codex/event");
    expect(packageManifest).not.toContain("@openai/codex-sdk");
  });

  it("Codex 使用通用 assistant message timeline helper", () => {
    expect(runnerSource).toContain("function emitAssistantMessageTimeline");
    expect(runnerSource).toMatch(/function emitAssistantMessageTimeline\(text, status, backend = "assistant"\)/);
    expect(runnerSource).toMatch(/sourceId:\s*`\$\{backend\}:text:message`/);
    expect(runnerSource).toContain('emit: (text) => emitAssistantMessageTimeline(text, "running", "codex")');
    expect(runnerSource).toContain("function emitCodexAssistantSuccess");
    expect(runnerSource).toContain("assistantSuccessEmitted");
    expect(runnerSource).toContain('emitAssistantMessageTimeline(text, "success", "codex")');
    expect(runnerSource).toContain("normalizeCodexAppServerEvent");
  });

  it("Codex 计划事件接入统一 plan pending 形态", () => {
    expect(runnerSource).toContain("maybeEmitCodexPlanApproval");
    expect(runnerSource).toContain('method === "turn/plan/updated"');
    expect(runnerSource).toContain("normalizeCodexPlanSteps");
    expect(runnerSource).toContain('type: "plan"');
    expect(runnerSource).toContain('kind: "plan"');
    expect(runnerSource).toContain('status: "requires_action"');
    expect(runnerSource).toContain("requestAskUser(buildPlanApprovalSpec()");
    expect(runnerSource).toContain("emitCodexPlanResolution");
    expect(runnerSource).not.toContain('getCodexItemType(item) !== "todo_list"');
  });

  it("Codex app-server v2 approval 通知接入统一工具授权", () => {
    expect(runnerSource).toContain("maybeHandleCodexServerRequest");
    expect(runnerSource).toMatch(/if\s*\(!entry\)\s*\{\s*notifications\.push\(msg\);/);
    expect(runnerSource).not.toContain('method === "applyPatchApproval"');
    expect(runnerSource).not.toContain('method === "execCommandApproval"');
    expect(runnerSource).not.toContain('decision: accepted ? "approved" : "denied"');
    expect(runnerSource).toContain('method === "item/commandExecution/requestApproval"');
    expect(runnerSource).toContain('method === "item/fileChange/requestApproval"');
    expect(runnerSource).toContain("normalizeCodexConsentTool");
    expect(runnerSource).toContain("requestUserConsent(payload)");
    expect(runnerSource).toContain('decision: accepted ? "accept" : "decline"');
  });

  it("Codex app-server turn/start 使用线程 sandbox 并正确结束失败 turn", () => {
    expect(runnerSource).toContain("sandbox: mapCodexSandboxMode(permission)");
    expect(runnerSource).not.toContain("sandboxPolicy: mapCodexSandboxMode");
    expect(runnerSource).toContain('const status = stringOrNull(turn?.status)');
    expect(runnerSource).toContain('status === "failed" || status === "interrupted"');
    expect(runnerSource).toContain('return { type: "turn.failed", ...params, error: turn?.error || params.error }');
    expect(runnerSource).toMatch(/if\s*\(type === "turn\.failed" \|\| type === "error"\)\s*\{[\s\S]*ctx\.turnCompletedSeen = true;/);
  });

  it("Codex 通过 app-server dynamic tool 和原生 requestUserInput 接入统一 AskUser", () => {
    expect(runnerSource).toContain("codexAskUserDynamicTool");
    expect(runnerSource).toContain('name: "AskUserQuestion"');
    expect(runnerSource).toContain("askUserQuestionJsonSchema");
    expect(runnerSource).toContain("dynamicTools: [codexAskUserDynamicTool]");
    expect(runnerSource).toContain('method === "item/tool/call"');
    expect(runnerSource).toContain('method === "item/tool/requestUserInput"');
    expect(runnerSource).toContain("codexRequestUserInputQuestionsToSpec");
    expect(runnerSource).toContain("askUserResultToCodexRequestUserInputResponse");
    expect(runnerSource).toContain('contentItems: [{ type: "inputText", text: JSON.stringify(output) }]');
    expect(runnerSource).toContain("server.respond(msg.id, askUserResultToCodexRequestUserInputResponse(result, spec))");
    expect(runnerSource).toContain('requestAskUser(spec, { backend: "codex" })');
    expect(runnerSource).toContain('backend === "codex" ? "codex" : "claude"');
    expect(runnerSource).not.toContain("startCodexAskUserBridge");
    expect(runnerSource).not.toContain("runCodexAskUserMcpServer");
    expect(runnerSource).not.toContain("codexAskUserConfigOverrides");
    expect(runnerSource).not.toContain('"mcp_servers.lilia.command"');
    expect(runnerSource).not.toContain('"mcp_servers.lilia.args"');
    expect(runnerSource).not.toContain("--codex-ask-user-mcp");
  });

  it("Codex app-server v2 事件不保留旧 SDK 兼容路径", () => {
    expect(runnerSource).toContain('method === "item/agentMessage/delta"');
    expect(runnerSource).toContain('type: "agentMessage.delta"');
    expect(runnerSource).toMatch(/if\s*\(type === "agentMessage\.delta"\)[\s\S]*ctx\.pacer\.push\(delta\)/);
    expect(runnerSource).not.toContain("codex.assistant_delta");
    expect(runnerSource).not.toContain('type: "item.updated"');
    expect(runnerSource).not.toContain("itemTextSeen");
    expect(runnerSource).not.toContain("pickCodexDeltaText");
    expect(runnerSource).not.toContain("agent_message");
    expect(runnerSource).not.toContain("assistant_message");
    expect(runnerSource).not.toContain("command_execution");
    expect(runnerSource).not.toContain("mcp_tool_call");
    expect(runnerSource).not.toContain("web_search");
    expect(runnerSource).not.toContain("item_type");
  });

  it("Codex MCP 配置发现事件带 config 子分类，真实 MCP 调用仍由 mcpToolCall 映射", () => {
    expect(runnerSource).toMatch(/case "mcpToolCall":\s*return \{ kind: "mcp" \};/);
    expect(runnerSource).toContain('title: "Codex MCP config"');
    expect(runnerSource).toMatch(/payload:\s*\{[\s\S]*?subkind:\s*"config"[\s\S]*?source:\s*"config\.toml"/);
  });
});
