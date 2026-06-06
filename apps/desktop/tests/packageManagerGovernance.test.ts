import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

function readRepoFile(path: string) {
  return readFileSync(resolve(repoRoot, path), "utf8");
}

describe("package manager governance", () => {
  it("keeps icon instructions on governed root yarn scripts", () => {
    const packageJson = JSON.parse(readRepoFile("package.json"));
    const publicInstructions = [
      readRepoFile("README.md"),
      readRepoFile("docs/guide/development.md"),
    ].join("\n");

    expect(packageJson.scripts["icons:generate"]).toMatch(/^yarn check:package-manager && /);
    expect(packageJson.scripts["icons:tauri"]).toMatch(/^yarn check:package-manager && /);
    expect(publicInstructions).toContain("yarn icons:generate");
    expect(publicInstructions).toContain("yarn icons:tauri");
    expect(publicInstructions).not.toContain("pwsh -File scripts/generate-icon.ps1");
    expect(publicInstructions).not.toContain(
      "yarn tauri icon apps/desktop/src-tauri/icons/icon-source.png",
    );
  });
});
