#!/usr/bin/env node

import { readFileSync } from "node:fs";

interface PackageManifest {
  engines?: {
    node?: string;
  };
  packageManager?: string;
}

const packageJson = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
) as PackageManifest;
const requiredNodeRange = packageJson.engines?.node ?? ">=26.0.0 <27";
const requiredPackageManager = packageJson.packageManager ?? "";
const requiredYarnVersion = parseYarnVersion(requiredPackageManager);
const currentNodeVersion = process.versions.node;
const userAgent = process.env.npm_config_user_agent ?? "";
const yarnVersion = userAgent.match(/\byarn\/([^\s]+)/)?.[1];

const problems: string[] = [];
if (Number.parseInt(currentNodeVersion.split(".")[0] ?? "", 10) !== 26) {
  problems.push(`Detected Node.js ${currentNodeVersion}.`);
}
if (!yarnVersion) {
  problems.push(
    userAgent
      ? `Detected package manager: ${userAgent}.`
      : "Could not detect the active package manager.",
  );
} else if (yarnVersion !== requiredYarnVersion) {
  problems.push(`Detected Yarn ${yarnVersion}.`);
}

if (problems.length === 0) {
  process.exit(0);
}

console.error(formatMessage(problems));
process.exit(1);

function parseYarnVersion(descriptor: string): string {
  const match = descriptor.match(/^yarn@([^+]+)(?:\+.+)?$/);
  if (!match?.[1]) {
    throw new Error(`Invalid packageManager descriptor: ${descriptor || "(missing)"}`);
  }
  return match[1];
}

function formatMessage(problems: string[]): string {
  return [
    "",
    "LiliaCode requires Node.js 26 and the pinned Yarn release through Corepack.",
    ...problems,
    "",
    `Expected Node.js: ${requiredNodeRange}`,
    `Expected package manager: ${requiredPackageManager}`,
    "",
    "Fix:",
    "  npm install --global corepack@0.35.0",
    "  corepack yarn install",
    "",
  ].join("\n");
}
