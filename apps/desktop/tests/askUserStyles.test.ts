import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const styles = readFileSync("src/styles.css", "utf8");

function selectorIndex(selector: string, fromIndex = 0): number {
  return styles.indexOf(selector, fromIndex);
}

function ruleTextAt(index: number): string {
  const end = styles.indexOf("}", index);
  return styles.slice(index, end + 1);
}

describe("ask user prompt styles", () => {
  it("作为 composer 内部扩展显示，不再依赖全局 search-palette 模态层", () => {
    const input = selectorIndex(".chat-composer__input {");
    const prompt = selectorIndex(".composer-inline {");
    const panel = selectorIndex(".chat-composer__pending-panel {");
    const panelInner = selectorIndex(".chat-composer__pending-panel-inner {");
    const entryRow = selectorIndex(".chat-composer__entry-row {");
    const entryActions = selectorIndex(".chat-composer__entry-actions {");
    const pendingActions = selectorIndex(".chat-composer__pending-actions {");
    const preview = selectorIndex(".composer-inline__main--with-preview {");
    const panelTransition = selectorIndex(".chat-composer-pending-panel-enter-active,");

    expect(input).toBeGreaterThan(-1);
    expect(prompt).toBeGreaterThan(-1);
    expect(panel).toBeGreaterThan(-1);
    expect(panelInner).toBeGreaterThan(panel);
    expect(entryRow).toBeGreaterThan(panelInner);
    expect(entryActions).toBeGreaterThan(entryRow);
    expect(pendingActions).toBeGreaterThan(entryActions);
    expect(preview).toBeGreaterThan(prompt);
    expect(panelTransition).toBeGreaterThan(preview);

    expect(styles).not.toContain("prefers-reduced-motion");
    expect(styles).not.toContain(".search-palette.ask-user");
    expect(styles).not.toContain(".ask-user {");
    expect(styles).not.toContain(".composer-inline__card");
    expect(styles).not.toContain(".chat-composer__mode");
    expect(styles).not.toContain(".chat-composer-mode");
    expect(styles).not.toContain(".chat-composer-inline-enter-active");
    expect(ruleTextAt(panelInner)).toContain("overflow: hidden");
    expect(ruleTextAt(panel)).toContain("display: grid");
    expect(ruleTextAt(panel)).toContain("grid-template-rows: 1fr");
    expect(ruleTextAt(panel)).toContain("overflow: hidden");
    expect(ruleTextAt(entryActions)).toContain("display: inline-flex");
    expect(ruleTextAt(prompt)).toContain("max-width: 100%");
    expect(ruleTextAt(prompt)).toContain("min-width: 0");
    expect(ruleTextAt(pendingActions)).toContain("display: inline-flex");
    expect(ruleTextAt(preview)).toContain("grid-template-columns");
    expect(ruleTextAt(panelTransition)).toContain("grid-template-rows 0.28s");
    expect(ruleTextAt(panelTransition)).toContain("opacity 0.18s");
  });

  it("计划确认扩展压缩为单行，为时间线计划正文留空间", () => {
    const compact = selectorIndex(".composer-inline--plan {");
    const entryRow = selectorIndex(".chat-composer__entry-row {");
    const pendingActions = selectorIndex(".chat-composer__pending-actions {");

    expect(compact).toBeGreaterThan(-1);
    expect(entryRow).toBeGreaterThan(-1);
    expect(pendingActions).toBeGreaterThan(entryRow);
    expect(ruleTextAt(compact)).toContain("flex-direction: row");
    expect(ruleTextAt(compact)).toContain("align-items: center");
    expect(ruleTextAt(entryRow)).toContain("align-items: flex-end");
    expect(ruleTextAt(pendingActions)).toContain("flex: 0 0 auto");
    expect(styles).not.toContain(".composer-inline--plan .composer-inline__actions");
  });

  it("右侧选项详情跟随选项列表高度，不截断选项列表", () => {
    const previewGrid = selectorIndex(".composer-inline__main--with-preview {");
    const preview = selectorIndex(".composer-inline__preview {");
    const previewPre = selectorIndex(".composer-inline__preview-pre {");
    const timelineOptionsClamp = selectorIndex(".timeline-pending-action .composer-inline__options {");

    expect(previewGrid).toBeGreaterThan(-1);
    expect(preview).toBeGreaterThan(previewGrid);
    expect(previewPre).toBeGreaterThan(preview);
    expect(timelineOptionsClamp).toBe(-1);

    expect(ruleTextAt(previewGrid)).toContain("align-items: stretch");
    expect(ruleTextAt(previewGrid)).not.toContain("clamp");
    expect(ruleTextAt(preview)).toContain("min-height: 0");
    expect(ruleTextAt(preview)).not.toContain("max-height");
    expect(ruleTextAt(previewPre)).toContain("overflow: auto");
  });

  it("时间线待处理输入框保持单行，聚焦不使用强调色描边", () => {
    const input = selectorIndex(".timeline-pending-action__input {");
    const focus = selectorIndex(".timeline-pending-action__input:focus {");

    expect(input).toBeGreaterThan(-1);
    expect(focus).toBeGreaterThan(input);
    expect(ruleTextAt(input)).toContain("height: 28px");
    expect(ruleTextAt(input)).toContain("resize: none");
    expect(ruleTextAt(input)).toContain("caret-color: var(--text-muted)");
    expect(ruleTextAt(focus)).toContain("outline: none");
    expect(ruleTextAt(focus)).toContain("border-color: var(--border-strong)");
    expect(ruleTextAt(focus)).not.toContain("var(--accent)");
  });

  it("富文本输入框空态 placeholder 不参与光标排版", () => {
    const input = selectorIndex(".chat-composer__rich-input {");
    const placeholder = selectorIndex(".chat-composer__rich-input.is-empty::before {");

    expect(input).toBeGreaterThan(-1);
    expect(placeholder).toBeGreaterThan(input);
    expect(ruleTextAt(input)).toContain("position: relative");
    expect(ruleTextAt(input)).toContain("caret-color: var(--text-muted)");
    expect(ruleTextAt(placeholder)).toContain("position: absolute");
    expect(ruleTextAt(placeholder)).toContain("inset: 4px 6px auto");
    expect(ruleTextAt(placeholder)).toContain("pointer-events: none");
  });

  it("其他回答输入框在操作行自然填满剩余空间", () => {
    const otherInput = selectorIndex(".composer-inline__other-input {");
    const timelineActions = selectorIndex(
      ".timeline-pending-action:not(.composer-inline--ask) .composer-inline__actions {",
    );

    expect(otherInput).toBeGreaterThan(-1);
    expect(timelineActions).toBeGreaterThan(otherInput);
    expect(ruleTextAt(otherInput)).toContain("flex: 1 1 auto");
    expect(ruleTextAt(otherInput)).toContain("min-width: 0");
    expect(ruleTextAt(otherInput)).toContain("width: auto");
    expect(ruleTextAt(otherInput)).not.toContain("max-width");
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
