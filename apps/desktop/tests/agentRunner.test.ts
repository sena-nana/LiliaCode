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

  it("Claude AskUserQuestion 不再先触发工具权限申请", () => {
    expect(runnerSource).toContain("LILIA_ASK_USER_TOOL_NAMES");
    expect(runnerSource).toMatch(
      /if \(isLiliaAskUserTool\(toolName\)\) \{\s*return \{ behavior: "allow", updatedInput: safeInput \};\s*\}/,
    );
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

  it("只读权限由 Lilia canUseTool 门禁拒绝可写或不可判定工具", () => {
    expect(runnerSource).toContain("isReadonlyDeniedClaudeTool");
    expect(runnerSource).toContain('ctx.executionPermission === "readonly"');
    expect(runnerSource).toContain("当前权限为只读，禁止写操作");
    expect(runnerSource).toContain("permissionDenied: true");
    expect(runnerSource).toContain('reason: "readonly"');
  });

  it("Claude thinking 与最终回复走互斥通道，由 block 类型权威路由", () => {
    // 行为细节走 claudeStreamDispatch.test.ts；这里只断言 runner 主文件确实
    // 把 stream_event 处理外包给了子模块 dispatcher，没有遗留 substring 猜测代码。
    expect(runnerSource).toContain("dispatchClaudeStreamEvent");
    expect(runnerSource).toContain("createClaudeStreamState");
    expect(runnerSource).toContain("finalizeClaudeReasoningBlocks");
    expect(runnerSource).not.toMatch(/isClaudeThinkingSummaryContainer/);
    expect(runnerSource).not.toMatch(/extractPublicClaudeThinkingSummary/);
    expect(runnerSource).not.toMatch(/thinkingByIndex/);
  });

  it("Claude text block 在 content_block_start 阶段提前占住 timeline order，避免被同 turn 的 tool_use 抢先", () => {
    // 根因：pacer 33ms 节流让 onTextDelta 的首次回调晚于 SDK 顶层 assistant 消息
    // 里的 tool_use emit，导致 message 的 INSERT 拿到比 Bash 更大的 order，UI 上
    // 短开场白被挤到工具后面。修复方式是 dispatcher 的 onTextStart 回调一收到，
    // runner 立刻 emit 一条空 running message 占住 sourceId / order，pacer 后续
    // 走同 sourceId 的 upsert 把内容填进去。
    const handler = runnerSource.match(
      /function handleClaudeStreamEvent\([\s\S]*?\n\}/,
    )?.[0];
    expect(handler).toBeTruthy();
    expect(handler).toMatch(/onTextStart:\s*\(\{\s*blockKey\s*\}\)\s*=>/);
    expect(handler).toMatch(
      /emitAssistantTextFragmentTimeline\(\s*"",\s*"running",\s*msg\?\.session_id,\s*blockKey\s*\)/,
    );
  });

  it("Claude result 阶段：finalText 缺失但出现过 text block 仍 emit 空 final 卡，便于发现 SDK 异常", () => {
    expect(runnerSource).toContain("sawAssistantTextBlock");
    expect(runnerSource).toMatch(/else if \(ctx\.sawAssistantTextBlock\)/);
  });

  it("Claude 工具结果只在出错时把 output 写进 summary，成功时让派生器从 payload 现算预览", () => {
    // 起始事件已把 command/path 写进 summary（normalizeClaudeTool 算出的），
    // 工具完成时如果再用 output 覆盖，折叠预览会从「指令」变「结果」——
    // 折叠态应稳定显示指令，输出留给 payload.output / 展开态。
    const branch = runnerSource.match(
      /function emitClaudeToolResultTimeline\([\s\S]*?\n\}/,
    )?.[0];
    expect(branch).toBeTruthy();
    expect(branch).toMatch(
      /const\s+summary\s*=\s*isError\s*&&\s*!askUserCancelled\s*\?\s*shortText\(text,\s*400\)[^;]*:\s*""/,
    );
    expect(branch).toMatch(
      /status:\s*askUserCancelled\s*\?\s*"cancelled"\s*:\s*isError\s*\?\s*"error"\s*:\s*"success"/,
    );
    expect(branch).not.toMatch(/const\s+summary\s*=\s*shortText\(text,\s*400\)\s*\|\|\s*""/);
  });

  it("Codex command 事件 summary 优先用指令，不被 aggregated_output 覆盖", () => {
    const branch = runnerSource.match(
      /function codexTimelineSummary\([\s\S]*?\n\}/,
    )?.[0];
    expect(branch).toBeTruthy();
    const commandCase = branch?.match(
      /case "command":[\s\S]*?(?=case "|default:|\}\s*$)/,
    )?.[0];
    expect(commandCase).toBeTruthy();
    expect(commandCase).toMatch(/shortText\(item\.command,\s*1200\)\s*\|\|/);
    expect(commandCase).not.toMatch(
      /shortText\(item\.aggregated_output[\s\S]*?\|\|\s*\n?\s*shortText\(item\.command/,
    );
  });

  it("本轮附件会作为路径上下文注入 Claude/Codex prompt", () => {
    expect(runnerSource).toContain("function buildPromptWithAttachments");
    expect(runnerSource).toContain("用户随本轮消息附加的本地路径");
    expect(runnerSource).toContain("不要假设已经读取了内容");
    expect(runnerSource).toMatch(/cmd\.prompt\s*=\s*buildPromptWithAttachments\(/);
    expect(runnerSource).toMatch(/attachments[\s\S]*?path/);
  });
});
