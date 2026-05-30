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
  it("作为 composer 内部扩展显示，不再依赖全局 search-palette 模态层", () => {
    const prompt = selectorIndex(".composer-inline {");
    const pending = selectorIndex(".chat-composer--pending {");
    const pendingActions = selectorIndex(".chat-composer__pending-actions {");
    const header = selectorIndex(".composer-inline__header {");
    const preview = selectorIndex(".composer-inline__main--with-preview {");
    const transition = selectorIndex(".chat-composer-inline-enter-active,");

    expect(prompt).toBeGreaterThan(-1);
    expect(pending).toBeGreaterThan(-1);
    expect(pendingActions).toBeGreaterThan(pending);
    expect(header).toBeGreaterThan(prompt);
    expect(preview).toBeGreaterThan(header);
    expect(transition).toBeGreaterThan(preview);

    expect(styles).not.toContain(".search-palette.ask-user");
    expect(styles).not.toContain(".ask-user {");
    expect(styles).not.toContain(".composer-inline__card");
    expect(ruleTextAt(prompt)).toContain("max-width: 100%");
    expect(ruleTextAt(prompt)).toContain("min-width: 0");
    expect(ruleTextAt(pending)).toContain("gap: 10px");
    expect(ruleTextAt(pendingActions)).toContain("display: inline-flex");
    expect(ruleTextAt(header)).toContain("grid-template-columns");
    expect(ruleTextAt(preview)).toContain("grid-template-columns");
    expect(ruleTextAt(transition)).toContain("transform 0.18s");
  });

  it("计划确认扩展压缩为单行，为时间线计划正文留空间", () => {
    const compact = selectorIndex(".composer-inline--plan {");
    const pendingRow = selectorIndex(".chat-composer__pending-row {");
    const pendingActions = selectorIndex(".chat-composer__pending-actions {");

    expect(compact).toBeGreaterThan(-1);
    expect(pendingRow).toBeGreaterThan(-1);
    expect(pendingActions).toBeGreaterThan(pendingRow);
    expect(ruleTextAt(compact)).toContain("flex-direction: row");
    expect(ruleTextAt(compact)).toContain("align-items: center");
    expect(ruleTextAt(pendingRow)).toContain("align-items: flex-end");
    expect(ruleTextAt(pendingActions)).toContain("flex: 0 0 auto");
    expect(styles).not.toContain(".composer-inline--plan .composer-inline__actions");
  });

  it("右侧选项详情使用稳定高度，切换选项时不改变面板高度", () => {
    const previewGrid = selectorIndex(".composer-inline__main--with-preview {");
    const previewSizing = selectorIndex(
      ".composer-inline__main--with-preview .composer-inline__options,",
    );
    const previewOptions = selectorIndex(
      ".composer-inline__main--with-preview .composer-inline__options {",
    );
    const previewPre = selectorIndex(".composer-inline__preview-pre {");

    expect(previewGrid).toBeGreaterThan(-1);
    expect(previewSizing).toBeGreaterThan(previewGrid);
    expect(previewOptions).toBeGreaterThan(previewSizing);
    expect(previewPre).toBeGreaterThan(previewOptions);

    expect(ruleTextAt(previewGrid)).toContain("--composer-inline-preview-height");
    expect(ruleTextAt(previewSizing)).toContain("height: var(--composer-inline-preview-height)");
    expect(ruleTextAt(previewSizing)).toContain("min-height: 0");
    expect(ruleTextAt(previewOptions)).toContain("overflow-y: auto");
    expect(ruleTextAt(previewPre)).toContain("overflow: auto");
  });

  it("推荐项只通过徽标提示，不给按钮默认描边", () => {
    const badge = selectorIndex(".composer-inline__badge {");
    const recommendedButton = selectorIndex(
      ".composer-inline__option.is-recommended .composer-inline__option-btn",
    );

    expect(badge).toBeGreaterThan(-1);
    expect(recommendedButton).toBe(-1);
  });
});
