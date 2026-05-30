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
  it("shell 左侧栏折叠使用列宽动画并禁用拖拽线", () => {
    const shell = rule(".shell {");
    const resizer = rule(".shell__resizer {");
    const collapsedResizer = rule(".shell.is-sidebar-collapsed .shell__resizer {");
    const panel = rule(".secondary-panel {");
    const collapsedPanel = rule(".shell.is-sidebar-collapsed .secondary-panel {");

    expect(shell).toContain("--sidebar-easing: cubic-bezier(0.4, 0, 0.2, 1)");
    expect(shell).toContain("transition: grid-template-columns 0.24s var(--sidebar-easing)");
    expect(resizer).toContain("transition:");
    expect(rule(".shell.is-resizing {")).toContain("transition: none");
    expect(rule(".shell.is-resizing .shell__resizer {")).toContain("transition: none");
    expect(collapsedResizer).toContain("pointer-events: none");
    expect(collapsedResizer).toContain("visibility: hidden");
    expect(panel).toContain("min-width: 0");
    expect(panel).toContain("transition:");
    expect(collapsedPanel).toContain("opacity: 0");
    expect(collapsedPanel).toContain("visibility: hidden");
    expect(collapsedPanel).toContain("pointer-events: none");
  });

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

  it("项目对话折叠省略行重置按钮外观", () => {
    const moreRow = rule(".sb-tree__row--more {");

    expect(moreRow).toContain("border: 0");
    expect(moreRow).toContain("appearance: none");
    expect(moreRow).toContain("background: transparent");
    expect(moreRow).toContain("text-align: left");
  });

  it("左下角按钮区跟随侧边栏底部四分之一区触发态显示", () => {
    const footerButtonsVisible = rule(".secondary-panel.is-footer-hot .sb-footer__btn,");
    const footerBadgeVisible = rule(".secondary-panel.is-footer-hot .sb-conn,");

    expect(footerButtonsVisible).toContain("opacity: 1");
    expect(footerBadgeVisible).toContain("opacity: 1");
  });
});
