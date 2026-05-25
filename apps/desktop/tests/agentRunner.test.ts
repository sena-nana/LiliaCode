import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { describe, expect, it } from "vitest";

const testsDir = dirname(fileURLToPath(import.meta.url));
const appRoot = join(testsDir, "..");
const runnerPath = join(appRoot, "agent-runner.mjs");
const claudeLoaderPath = join(testsDir, "fixtures", "agent-runner-claude-loader.mjs");

describe("agent-runner Claude stream", () => {
  it("不会把同一个 Claude 文本 delta 重复推送到 running timeline", async () => {
    const events = await runAgentRunner({
      backend: "claude",
      cwd: appRoot,
      prompt: "请回复你好",
      permission: "ask",
    });

    const runningContents = events
      .filter((event) => event.type === "timeline")
      .map((event) => event.event)
      .filter((event) =>
        event.kind === "message" &&
        event.status === "running" &&
        event.payload?.role === "assistant"
      )
      .map((event) => event.payload?.content);

    expect(runningContents).toContain("你好");
    expect(runningContents).not.toContain("你你好好");
  });
});

function runAgentRunner(command: Record<string, unknown>): Promise<Array<Record<string, any>>> {
  return new Promise((resolve, reject) => {
    const child = spawn(
      process.execPath,
      ["--experimental-loader", pathToFileURL(claudeLoaderPath).href, runnerPath],
      {
        cwd: appRoot,
        env: {
          ...process.env,
          NODE_NO_WARNINGS: "1",
        },
        stdio: ["pipe", "pipe", "pipe"],
      },
    );

    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0) {
        reject(new Error(`agent-runner exited ${code}\n${stderr}\n${stdout}`));
        return;
      }

      try {
        resolve(
          stdout
            .split(/\r?\n/)
            .map((line) => line.trim())
            .filter(Boolean)
            .map((line) => JSON.parse(line)),
        );
      } catch (err) {
        reject(err);
      }
    });

    child.stdin.end(JSON.stringify(command));
  });
}
