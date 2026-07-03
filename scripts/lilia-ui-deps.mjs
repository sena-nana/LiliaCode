#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const rootManifestPath = resolve(repoRoot, "package.json");
const localRoot = resolve(repoRoot, process.env.LILIA_UI_LOCAL_PATH || "../LiliaUI");

const packages = [
  ["@lilia/build", "packages/build"],
  ["@lilia/config", "packages/config"],
  ["@lilia/tools", "packages/tools"],
  ["@lilia/ui", "packages/ui"],
];

const mode = process.argv[2] || "status";
if (!["local", "remote", "status"].includes(mode)) {
  fail("Usage: yarn liliaui:local | yarn liliaui:remote | yarn liliaui:status");
}

if (mode === "status") {
  printStatus();
  process.exit(0);
}

const localPackageRoots = packages.map(([, path]) => resolve(localRoot, path));
if (mode === "local") {
  assertLocalPackages(localPackageRoots);
}

runYarn(
  mode === "local"
    ? ["link", ...localPackageRoots, "--relative"]
    : ["unlink", ...localPackageRoots],
);
printStatus();

function assertLocalPackages(packageRoots) {
  for (let index = 0; index < packages.length; index += 1) {
    const [name] = packages[index];
    const manifestPath = resolve(packageRoots[index], "package.json");
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    if (manifest.name !== name) {
      fail(`Expected ${manifestPath} to declare ${name}, got ${manifest.name || "(missing)"}.`);
    }
  }
}

function runYarn(args) {
  const yarnJs = resolveYarnJs(process.env.npm_execpath);
  if (!yarnJs) {
    fail("Run this script through a root Yarn command, for example: yarn liliaui:local");
  }

  const result = spawnSync(process.execPath, [yarnJs, ...args], {
    cwd: repoRoot,
    stdio: "inherit",
    env: process.env,
  });

  if (result.error) fail(result.error.message);
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function printStatus() {
  const resolutions = readRootManifest().resolutions ?? {};
  console.log("LiliaUI dependency source:");
  for (const [name] of packages) {
    const resolution = resolutions[name];
    const source =
      typeof resolution === "string" && resolution.startsWith("portal:")
        ? `local (${resolution})`
        : "remote";
    console.log(`  ${name}: ${source}`);
  }
}

function readRootManifest() {
  return JSON.parse(readFileSync(rootManifestPath, "utf8"));
}

function resolveYarnJs(yarnCli) {
  if (!yarnCli) return null;
  if (/\.[cm]?js$/i.test(yarnCli)) return yarnCli;

  const content = readFileSync(yarnCli, "utf8");
  if (content.startsWith("#!/usr/bin/env node")) return yarnCli;

  return content.match(/['"]([^'"]*corepack[^'"]*yarn\.js)['"]/i)?.[1] ?? null;
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
