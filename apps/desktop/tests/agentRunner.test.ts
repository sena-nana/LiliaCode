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
});
