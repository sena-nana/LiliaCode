#!/usr/bin/env node

import { readFileSync } from "node:fs";

const packageJson = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);
const requiredPackageManager = packageJson.packageManager;
const userAgent = process.env.npm_config_user_agent ?? "";

const yarnMatch = userAgent.match(/\byarn\/([^\s]+)/);
const yarnVersion = yarnMatch?.[1];
const yarnMajor = Number.parseInt(yarnVersion?.split(".")[0] ?? "", 10);

if (yarnMajor >= 4) {
  process.exit(0);
}

const reason = yarnVersion
  ? `Detected Yarn ${yarnVersion}.`
  : userAgent
    ? `Detected package manager: ${userAgent}.`
    : "Could not detect the active package manager.";

console.error(formatMessage(reason));
process.exit(1);

function formatMessage(reason) {
  return [
    "",
    "LiliaCode requires Yarn 4 through Corepack.",
    reason,
    "",
    `Expected package manager: ${requiredPackageManager}`,
    "",
    "Fix:",
    "  corepack enable",
    `  corepack prepare ${requiredPackageManager} --activate`,
    "  yarn install",
    "",
    "If the `yarn` command still resolves to Yarn 1, run the commands through Corepack:",
    "  corepack yarn install",
    "  corepack yarn dev",
    "",
  ].join("\n");
}
