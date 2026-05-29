import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const styles = readFileSync("src/styles.css", "utf8");

function selectorIndex(selector: string): number {
  return styles.indexOf(selector);
}

function ruleTextAt(index: number): string {
  const end = styles.indexOf("}", index);
  return styles.slice(index, end + 1);
}

describe("ask user prompt styles", () => {
  it("作为对话内面板显示，不再依赖全局 search-palette 模态层", () => {
    const prompt = selectorIndex(".ask-user {");
    const header = selectorIndex(".ask-user__header {");
    const preview = selectorIndex(".ask-user__main--with-preview {");
    const transition = selectorIndex(".ask-user-enter-active,");

    expect(prompt).toBeGreaterThan(-1);
    expect(header).toBeGreaterThan(prompt);
    expect(preview).toBeGreaterThan(header);
    expect(transition).toBeGreaterThan(preview);

    expect(styles).not.toContain(".search-palette.ask-user");
    expect(styles).not.toContain(".ask-user__card");
    expect(ruleTextAt(prompt)).toContain("max-width: 100%");
    expect(ruleTextAt(prompt)).toContain("min-width: 0");
    expect(ruleTextAt(prompt)).toContain("background: var(--bg-elev)");
    expect(ruleTextAt(prompt)).toContain("border-radius: 14px");
    expect(ruleTextAt(prompt)).toContain("box-shadow: 0 4px 16px -8px");
    expect(ruleTextAt(header)).toContain("grid-template-columns");
    expect(ruleTextAt(preview)).toContain("grid-template-columns");
    expect(ruleTextAt(transition)).toContain("transform 0.18s");
  });

  it("计划确认面板压缩为单行，为时间线计划正文留空间", () => {
    const compact = selectorIndex(".ask-user--plan-approval {");
    const compactActions = selectorIndex(".ask-user--plan-approval .ask-user__actions {");

    expect(compact).toBeGreaterThan(-1);
    expect(compactActions).toBeGreaterThan(compact);
    expect(ruleTextAt(compact)).toContain("flex-direction: row");
    expect(ruleTextAt(compact)).toContain("align-items: center");
    expect(ruleTextAt(compactActions)).toContain("flex: 0 0 auto");
    expect(ruleTextAt(compactActions)).toContain("margin-left: auto");
  });
});
