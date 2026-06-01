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
  it("展开的长入参在 composer 内部换行，不把聊天区撑宽", () => {
    const inline = selectorIndex(".composer-inline {");
    const row = selectorIndex(".composer-inline__tool-row {");
    const details = selectorIndex(".composer-inline__details {");

    expect(inline).toBeGreaterThan(-1);
    expect(row).toBeGreaterThan(inline);
    expect(details).toBeGreaterThan(row);

    expect(styles).not.toContain(".tool-consent {");
    expect(ruleTextAt(inline)).toContain("max-width: 100%");
    expect(ruleTextAt(inline)).toContain("min-width: 0");
    expect(ruleTextAt(inline)).toContain("gap: 10px");
    expect(ruleTextAt(row)).toContain("min-width: 0");
    expect(ruleTextAt(details)).toContain("min-width: 0");
    expect(ruleTextAt(details)).toContain("max-width: 100%");
    expect(ruleTextAt(details)).toContain("padding: 8px 10px");
    expect(ruleTextAt(details)).toContain("border-radius: 6px");
    expect(ruleTextAt(details)).toContain("line-height: 1.45");
    expect(ruleTextAt(details)).toContain("max-height: 176px");
    expect(ruleTextAt(details)).toContain("white-space: pre-wrap");
    expect(ruleTextAt(details)).toContain("overflow-wrap: anywhere");
  });

  it("可编辑命令框沿用克制的代码块样式并支持长命令换行", () => {
    const wrapper = selectorIndex(".editable-command {");
    const preview = selectorIndex(".editable-command__preview {");
    const input = selectorIndex(".editable-command__input {");
    const hint = selectorIndex(".editable-command__hint {");

    expect(wrapper).toBeGreaterThan(-1);
    expect(preview).toBeGreaterThan(wrapper);
    expect(input).toBeGreaterThan(preview);
    expect(hint).toBeGreaterThan(input);

    expect(ruleTextAt(preview)).toContain("white-space: pre-wrap");
    expect(ruleTextAt(preview)).toContain("overflow-wrap: anywhere");
    expect(ruleTextAt(input)).toContain("resize: vertical");
    expect(ruleTextAt(input)).toContain("white-space: pre-wrap");
    expect(ruleTextAt(input)).toContain("overflow-wrap: anywhere");
    expect(ruleTextAt(hint)).toContain("color: var(--text-muted)");
  });
});
