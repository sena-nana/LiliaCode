import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const styles = readFileSync("src/styles.css", "utf8");

function selectorIndex(selector: string): number {
  const match = new RegExp(`(^|\\n)${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`).exec(styles);
  return match?.index ?? -1;
}

function rule(selector: string): string {
  const index = selectorIndex(selector);
  expect(index).toBeGreaterThan(-1);
  const end = styles.indexOf("}", index);
  return styles.slice(index, end + 1);
}

describe("sidebar project tree styles", () => {
  it("项目区的新对话、项目名和对话标题共用同一文本列，收集箱保持独立缩进", () => {
    const panel = rule(".secondary-panel {");
    const primaryButton = rule(".sb-primary-btn {");
    const projectRow = rule(".sb-tree__row--project {");
    const projectLink = rule(".sb-tree__link {");
    const projectIcon = rule(".sb-tree__project-icon {");
    const childRow = rule(".sb-tree__row--child {");
    const orphanRow = rule(".sb-tree__row--orphan {");

    expect(panel).toContain("--sb-row-padding-x: 10px");
    expect(panel).toContain("--sb-project-row-padding-x: 6px");
    expect(panel).toContain("--sb-row-gap: 6px");
    expect(panel).toContain("--sb-project-text-start: 30px");
    expect(panel).toContain("--sb-project-icon-slot: calc(var(--sb-project-text-start) - var(--sb-project-row-padding-x) - var(--sb-row-gap))");
    expect(primaryButton).toContain("padding: 0 var(--sb-row-padding-x) 0 var(--sb-project-row-padding-x)");
    expect(primaryButton).toContain("gap: var(--sb-row-gap)");
    expect(primaryButton).toContain("grid-template-columns: var(--sb-project-icon-slot) minmax(0, 1fr)");
    expect(projectRow).toContain("padding-left: var(--sb-project-row-padding-x)");
    expect(projectLink).toContain("gap: var(--sb-row-gap)");
    expect(projectIcon).toContain("flex: 0 0 var(--sb-project-icon-slot)");
    expect(projectIcon).toContain("width: var(--sb-project-icon-slot)");
    expect(childRow).toContain("padding-left: var(--sb-project-text-start)");
    expect(orphanRow).toContain("padding-left: 8px");
    expect(orphanRow).not.toContain("--sb-project-text-start");
  });
});
