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

describe("tool consent prompt styles", () => {
  it("展开的长入参在授权卡片内部换行，不把聊天区撑宽", () => {
    const card = selectorIndex(".tool-consent {");
    const row = selectorIndex(".tool-consent__row {");
    const details = selectorIndex(".tool-consent__details {");

    expect(card).toBeGreaterThan(-1);
    expect(row).toBeGreaterThan(card);
    expect(details).toBeGreaterThan(row);

    expect(ruleTextAt(card)).toContain("max-width: 100%");
    expect(ruleTextAt(card)).toContain("min-width: 0");
    expect(ruleTextAt(row)).toContain("min-width: 0");
    expect(ruleTextAt(details)).toContain("min-width: 0");
    expect(ruleTextAt(details)).toContain("max-width: 100%");
    expect(ruleTextAt(details)).toContain("white-space: pre-wrap");
    expect(ruleTextAt(details)).toContain("overflow-wrap: anywhere");
  });
});
