#!/usr/bin/env node

import { spawn, spawnSync } from "node:child_process";

const args = process.argv.slice(2);

if (args.length === 0) {
  console.error("Usage: node scripts/with-rust-cache.mjs <command> [...args]");
  process.exit(1);
}

const [command, ...commandArgs] = args;
const env = createRustCacheEnv(process.env);
const child = run(command, commandArgs, {
  cwd: process.cwd(),
  env,
  stdio: "inherit",
});

child.on("error", (error) => {
  console.error(error?.message ?? String(error));
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});

function createRustCacheEnv(baseEnv) {
  const nextEnv = { ...baseEnv };
  if (isDisabled(nextEnv.LILIA_RUST_CACHE)) return nextEnv;
  if (String(nextEnv.RUSTC_WRAPPER ?? "").trim()) return nextEnv;
  if (commandExists("sccache", ["--version"], nextEnv)) {
    nextEnv.RUSTC_WRAPPER = "sccache";
  }
  return nextEnv;
}

function isDisabled(value) {
  return /^(0|false|off|no)$/i.test(String(value ?? "").trim());
}

function commandExists(candidate, candidateArgs, env) {
  const result = runSync(candidate, candidateArgs, {
    env,
    stdio: "ignore",
  });
  return result.status === 0;
}

function run(candidate, candidateArgs, options) {
  const [resolvedCommand, resolvedArgs] = resolveSpawn(candidate, candidateArgs);
  return spawn(resolvedCommand, resolvedArgs, options);
}

function runSync(candidate, candidateArgs, options) {
  const [resolvedCommand, resolvedArgs] = resolveSpawn(candidate, candidateArgs);
  return spawnSync(resolvedCommand, resolvedArgs, options);
}

function resolveSpawn(candidate, candidateArgs) {
  if (process.platform === "win32" && candidate.toLowerCase() === "yarn") {
    return ["cmd.exe", ["/d", "/s", "/c", "call", "yarn", ...candidateArgs]];
  }
  return [candidate, candidateArgs];
}
