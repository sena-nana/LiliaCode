import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const testsDir = dirname(fileURLToPath(import.meta.url));
const runnerSource = readFileSync(join(testsDir, "..", "agent-runner.mjs"), "utf8");

describe("agent-runner Claude stream", () => {
  it("只把 Claude 文本 delta 推进 pacer 一次", () => {
    const streamEventBranch = runnerSource.match(
      /case "stream_event": \{([\s\S]*?)\n\s*break;\n\s*\}/,
    )?.[1];

    expect(streamEventBranch).toContain("ctx.assistantDeltaText += text;");
    expect(streamEventBranch?.match(/pacer\.push\(text\);/g) ?? []).toHaveLength(1);
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
});
