#!/usr/bin/env node

const requiredPackageManager = "yarn@4.14.1";
const userAgent = process.env.npm_config_user_agent ?? "";

const yarnMatch = userAgent.match(/\byarn\/([^\s]+)/);

if (yarnMatch) {
  const version = yarnMatch[1];
  const major = Number.parseInt(version.split(".")[0] ?? "", 10);

  if (major >= 4) {
    process.exit(0);
  }

  console.error(formatMessage(`Detected Yarn ${version}.`));
  process.exit(1);
}

if (userAgent) {
  console.error(formatMessage(`Detected package manager: ${userAgent}.`));
  process.exit(1);
}

console.error(formatMessage("Could not detect the active package manager."));
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
    "  corepack prepare yarn@4.14.1 --activate",
    "  yarn install",
    "",
    "If the `yarn` command still resolves to Yarn 1, run the commands through Corepack:",
    "  corepack yarn install",
    "  corepack yarn dev",
    "",
  ].join("\n");
}
