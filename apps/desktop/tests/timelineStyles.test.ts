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

describe("agent timeline styles", () => {
  it("聊天滚动容器占满界面宽度，滚动条停在界面右侧", () => {
    const chat = selectorIndex(".chat {");
    const controls = selectorIndex(".chat > :not(.chat-transcript) {");

    expect(chat).toBeGreaterThan(-1);
    expect(controls).toBeGreaterThan(chat);
    expect(ruleTextAt(chat)).toContain("width: 100%");
    expect(ruleTextAt(chat)).not.toContain("max-width: 860px");
    expect(ruleTextAt(chat)).not.toContain("margin: 0 auto");
    expect(ruleTextAt(controls)).toContain("width: min(100%, 860px)");
    expect(ruleTextAt(controls)).toContain("justify-self: center");
  });

  it("内容列右侧保留与左侧时间线槽位对应的补偿边距", () => {
    const timeline = selectorIndex(".agent-timeline {");

    expect(timeline).toBeGreaterThan(-1);
    expect(ruleTextAt(timeline)).toContain("--agent-timeline-rail-offset: 28px");
    expect(ruleTextAt(timeline)).toContain("width: min(100%, calc(760px + var(--agent-timeline-rail-offset)))");
    expect(ruleTextAt(timeline)).toContain("padding-right: var(--agent-timeline-rail-offset)");
  });

  it("Agent 最终回复正文和代码片段都使用更高字重", () => {
    const finalReply = selectorIndex(".timeline-card--final-reply .markdown-block");
    const finalReplyCode = selectorIndex(".timeline-card--final-reply .markdown-block code");

    expect(finalReply).toBeGreaterThan(-1);
    expect(finalReplyCode).toBeGreaterThan(finalReply);
    expect(ruleTextAt(finalReply)).toContain("font-weight: 500");
    expect(ruleTextAt(finalReplyCode)).toContain("font-weight: 500");
  });

  it("折叠项 hover 时只高亮箭头，不改写标题文本颜色", () => {
    const titleHover = selectorIndex(".agent-timeline__title:hover:not(:disabled)");
    const chevronHover = selectorIndex(".agent-timeline__title:hover:not(:disabled) .agent-timeline__chevron");
    const failedChevronHover = selectorIndex(
      ".agent-timeline__item:is(.is-status-failed, .is-status-error, .is-status-cancelled) .agent-timeline__title:hover:not(:disabled) .agent-timeline__chevron",
    );

    expect(titleHover).toBeGreaterThan(-1);
    expect(chevronHover).toBeGreaterThan(titleHover);
    expect(failedChevronHover).toBeGreaterThan(chevronHover);
    expect(ruleTextAt(titleHover)).not.toContain("color: var(--accent)");
    expect(ruleTextAt(chevronHover)).toContain("color: var(--accent)");
    expect(ruleTextAt(failedChevronHover)).toContain("color: var(--err)");
  });

  it("失败折叠项 hover 时继续使用错误色", () => {
    const titleHover = selectorIndex(".agent-timeline__title:hover:not(:disabled)");
    const failedTitleHover = selectorIndex(
      ".agent-timeline__item:is(.is-status-failed, .is-status-error, .is-status-cancelled) .agent-timeline__title:hover:not(:disabled)",
    );
    const processHover = selectorIndex(".agent-timeline__process-toggle:hover");
    const failedProcessHover = selectorIndex(".agent-timeline__process-toggle--failed:hover");

    expect(titleHover).toBeGreaterThan(-1);
    expect(processHover).toBeGreaterThan(-1);
    expect(failedTitleHover).toBeGreaterThan(titleHover);
    expect(failedProcessHover).toBeGreaterThan(processHover);
    expect(ruleTextAt(failedTitleHover)).toContain("color: var(--err)");
    expect(ruleTextAt(failedProcessHover)).toContain("color: var(--err)");
  });
});
