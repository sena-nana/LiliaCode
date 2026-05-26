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

  it("Claude thinking delta 走 reasoning timeline，并按 block index 累加文本", () => {
    // 抽取 candidate 字段时把 thinking 排到首位，否则 thinking_delta
    // 的实际载体 (`delta.thinking`) 会被漏掉。
    expect(runnerSource).toMatch(
      /candidate\.thinking\s*\|\|[\s\S]*?candidate\.summary/,
    );

    // 同一 block 的多个 delta 必须聚合：sourceId 与 block index 绑定，
    // ctx.thinkingByIndex 持久化已发文本，新 delta 拼接后 upsert 同一事件。
    expect(runnerSource).toContain("thinkingByIndex");
    expect(runnerSource).toMatch(/`\$\{[^`]+\}:thinking:\$\{index\}`/);

    // turn 结束要把所有 thinking 块标记完成，让标题从「正在思考」→「已思考」。
    expect(runnerSource).toContain("finalizeClaudeThinkingBlocks");
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
      /const\s+summary\s*=\s*isError\s*\?\s*shortText\(text,\s*400\)[^;]*:\s*""/,
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
});
