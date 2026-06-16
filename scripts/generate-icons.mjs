#!/usr/bin/env node

import { rmSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("..", import.meta.url));
const iconsDir = fileURLToPath(new URL("../apps/desktop/src-tauri/icons/", import.meta.url));

const result =
  process.platform === "win32"
    ? spawnSync(
        "cmd.exe",
        ["/d", "/s", "/c", "yarn --cwd apps/desktop tauri icon src-tauri/icons/icon.png"],
        {
          cwd: repoRoot,
          encoding: "utf8",
        },
      )
    : spawnSync(
        "yarn",
        ["--cwd", "apps/desktop", "tauri", "icon", "src-tauri/icons/icon.png"],
        {
          cwd: repoRoot,
          encoding: "utf8",
        },
      );

if (result.stdout) {
  process.stdout.write(result.stdout);
}
if (result.stderr) {
  process.stderr.write(result.stderr);
}
if (result.error) {
  console.error(result.error.message);
}
if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

const generatedExtras = [
  "64x64.png",
  "icon.icns",
  "Square107x107Logo.png",
  "Square142x142Logo.png",
  "Square150x150Logo.png",
  "Square284x284Logo.png",
  "Square30x30Logo.png",
  "Square310x310Logo.png",
  "Square44x44Logo.png",
  "Square71x71Logo.png",
  "Square89x89Logo.png",
  "StoreLogo.png",
  "android",
  "ios",
];

for (const entry of generatedExtras) {
  rmSync(join(iconsDir, entry), {
    force: true,
    recursive: true,
  });
}

console.log(
  `Kept desktop icon assets in ${iconsDir}`,
);
