// Lilia · Agent runner CLI entry.
//
// Rust/Tauri still launches this file directly with `node agent-runner.mjs`.
// The implementation lives under `agent-runner/` so this file only wires stdio
// and process lifetime.

import { createInterface } from "node:readline";
import { createRunnerContext, runAgentTurn } from "./agent-runner/core.mjs";

async function readInitialCommand(rl, protocol, context) {
  let cmd = null;
  let resolveCommand;
  const ready = new Promise((resolve) => {
    resolveCommand = resolve;
  });

  rl.on("line", (line) => {
    if (!cmd) {
      try {
        cmd = JSON.parse(line.replace(/^\uFEFF/, ""));
      } catch (err) {
        protocol.emit({ type: "error", message: `invalid stdin JSON: ${err.message}` });
        process.exit(1);
      }
      resolveCommand(cmd);
      return;
    }
    context.interactions.handleControlLine(line);
  });

  return ready;
}

async function main() {
  const context = createRunnerContext();
  const rl = createInterface({ input: process.stdin });
  const cmd = await readInitialCommand(rl, context.protocol, context);
  const result = await runAgentTurn(cmd, context);
  rl.close();
  process.exit(result.exitCode);
}

main().catch((err) => {
  const context = createRunnerContext();
  context.protocol.emit({ type: "error", message: err?.message || String(err) });
  process.exit(1);
});
