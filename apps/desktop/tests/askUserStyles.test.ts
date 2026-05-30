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
    const header = selectorIndex(".composer-inline__header {");
    const preview = selectorIndex(".composer-inline__main--with-preview {");
    const panelTransition = selectorIndex(".chat-composer-pending-panel-enter-active,");
    const panelFrom = selectorIndex(".chat-composer-pending-panel-enter-from,");
    const panelTo = selectorIndex(".chat-composer-pending-panel-enter-to,");
    const entryActionsTransition = selectorIndex(".chat-composer-entry-actions-enter-active,");
    const entryActionsFrom = selectorIndex(".chat-composer-entry-actions-enter-from,");
    const entryActionsTo = selectorIndex(".chat-composer-entry-actions-enter-to,");
    const stackTransition = selectorIndex(".chat-composer-stack-enter-active,");
    const stackFrom = selectorIndex(".chat-composer-stack-enter-from,");
    const stackTo = selectorIndex(".chat-composer-stack-enter-to,");
    const reducedPanelTransition = selectorIndex(
      ".chat-composer-pending-panel-enter-active,",
      panelTransition + 1,
    );

    expect(input).toBeGreaterThan(-1);
    expect(prompt).toBeGreaterThan(-1);
    expect(panel).toBeGreaterThan(-1);
    expect(panelInner).toBeGreaterThan(panel);
    expect(entryRow).toBeGreaterThan(panelInner);
    expect(entryActions).toBeGreaterThan(entryRow);
    expect(pendingActions).toBeGreaterThan(entryActions);
    expect(header).toBeGreaterThan(prompt);
    expect(preview).toBeGreaterThan(header);
    expect(panelTransition).toBeGreaterThan(preview);
    expect(panelFrom).toBeGreaterThan(panelTransition);
    expect(panelTo).toBeGreaterThan(panelFrom);
    expect(entryActionsTransition).toBeGreaterThan(panelTo);
    expect(entryActionsFrom).toBeGreaterThan(entryActionsTransition);
    expect(entryActionsTo).toBeGreaterThan(entryActionsFrom);
    expect(stackTransition).toBeGreaterThan(entryActionsTo);
    expect(stackFrom).toBeGreaterThan(stackTransition);
    expect(stackTo).toBeGreaterThan(stackFrom);
    expect(reducedPanelTransition).toBeGreaterThan(stackTo);

    expect(styles).not.toContain(".search-palette.ask-user");
    expect(styles).not.toContain(".ask-user {");
    expect(styles).not.toContain(".composer-inline__card");
    expect(styles).not.toContain(".chat-composer__mode");
    expect(styles).not.toContain(".chat-composer-mode");
    expect(styles).not.toContain(".chat-composer-inline-enter-active");
    expect(ruleTextAt(input)).toContain("transition: height 0.16s");
    expect(ruleTextAt(panelInner)).toContain("overflow: hidden");
    expect(ruleTextAt(panel)).toContain("display: grid");
    expect(ruleTextAt(panel)).toContain("grid-template-rows: 1fr");
    expect(ruleTextAt(panel)).toContain("overflow: hidden");
    expect(ruleTextAt(entryRow)).toContain("display: flex");
    expect(ruleTextAt(entryActions)).toContain("display: inline-flex");
    expect(ruleTextAt(prompt)).toContain("max-width: 100%");
    expect(ruleTextAt(prompt)).toContain("min-width: 0");
    expect(ruleTextAt(pendingActions)).toContain("display: inline-flex");
    expect(ruleTextAt(header)).toContain("grid-template-columns");
    expect(ruleTextAt(preview)).toContain("grid-template-columns");
    expect(ruleTextAt(panelTransition)).toContain("grid-template-rows 0.28s");
    expect(ruleTextAt(panelTransition)).toContain("opacity 0.18s");
    expect(ruleTextAt(panelTransition)).toContain("will-change: grid-template-rows, opacity");
    expect(ruleTextAt(panelFrom)).toContain("grid-template-rows: 0fr");
    expect(ruleTextAt(panelFrom)).toContain("opacity: 0");
    expect(ruleTextAt(panelTo)).toContain("grid-template-rows: 1fr");
    expect(ruleTextAt(panelTo)).toContain("opacity: 1");
    expect(ruleTextAt(entryActionsTransition)).toContain("max-width 0.32s");
    expect(ruleTextAt(entryActionsTransition)).toContain("opacity 0.18s");
    expect(ruleTextAt(entryActionsFrom)).toContain("max-width: 0");
    expect(ruleTextAt(entryActionsTo)).toContain("max-width: 520px");
    expect(ruleTextAt(stackTransition)).toContain("max-height 0.32s");
    expect(ruleTextAt(stackTransition)).toContain("opacity 0.18s");
    expect(ruleTextAt(stackFrom)).toContain("max-height: 0");
    expect(ruleTextAt(stackTo)).toContain("max-height: 260px");
    expect(ruleTextAt(reducedPanelTransition)).toContain("transition: none");
    expect(ruleTextAt(reducedPanelTransition)).toContain("will-change: auto");
    expect(ruleTextAt(reducedPanelTransition)).toContain(".chat-composer-entry-actions-enter-active");
    expect(ruleTextAt(reducedPanelTransition)).toContain(".chat-composer-stack-enter-active");
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
