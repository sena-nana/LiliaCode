import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const styles = readFileSync("src/styles.css", "utf8");
const chatTranscript = readFileSync("src/components/chat/ChatTranscript.vue", "utf8");
const taskDetail = readFileSync("src/pages/TaskDetail.vue", "utf8");

function selectorIndex(selector: string): number {
  return styles.indexOf(selector);
}

function selectorLastIndex(selector: string): number {
  return styles.lastIndexOf(selector);
}

function ruleTextAt(index: number): string {
  const end = styles.indexOf("}", index);
  return styles.slice(index, end + 1);
}

describe("agent timeline styles", () => {
  it("聊天滚动容器占满界面宽度，滚动条停在界面右侧", () => {
    const chat = selectorIndex(".chat {");
    const controls = selectorIndex(".chat-controls {");

    expect(chat).toBeGreaterThan(-1);
    expect(controls).toBeGreaterThan(chat);
    expect(ruleTextAt(chat)).toContain("width: 100%");
    expect(ruleTextAt(chat)).not.toContain("max-width: 860px");
    expect(ruleTextAt(chat)).not.toContain("margin: 0 auto");
    expect(ruleTextAt(controls)).toContain("width: min(100%, 860px)");
    expect(ruleTextAt(controls)).toContain("align-self: center");
    expect(taskDetail).toContain('class="chat-controls"');
  });

  it("聊天滚动范围延伸到界面底部", () => {
    const chat = selectorIndex(".chat {");
    const transcript = selectorIndex(".chat-transcript {");
    const controls = selectorIndex(".chat-controls {");
    const controlsWrap = selectorIndex(".chat-controls-wrap {");

    expect(chat).toBeGreaterThan(-1);
    expect(transcript).toBeGreaterThan(chat);
    expect(controls).toBeGreaterThan(chat);
    expect(controlsWrap).toBeGreaterThan(transcript);
    expect(ruleTextAt(chat)).not.toContain("overflow-y: auto");
    expect(ruleTextAt(transcript)).toContain("overflow-y: auto");
    expect(ruleTextAt(transcript)).toContain("padding: 8px 4px 0");
    expect(ruleTextAt(controls)).toContain("align-self: center");
    expect(ruleTextAt(controlsWrap)).toContain("position: sticky");
    expect(ruleTextAt(controlsWrap)).toContain("bottom: 0");
    expect(chatTranscript).toContain('<slot name="controls" />');
    expect(chatTranscript).toContain('class="chat-controls-wrap"');
  });

  it("输入区粘底时遮住底部安全区，时间线不会露到输入框下方", () => {
    const transcript = selectorIndex(".chat-transcript {");
    const controlsWrap = selectorIndex(".chat-controls-wrap {");

    expect(transcript).toBeGreaterThan(-1);
    expect(controlsWrap).toBeGreaterThan(transcript);
    expect(ruleTextAt(transcript)).toContain("gap: 0");
    expect(ruleTextAt(transcript)).toContain("padding: 8px 4px 0");
    expect(ruleTextAt(controlsWrap)).toContain("margin: auto -4px 0");
    expect(ruleTextAt(controlsWrap)).toContain("padding: 12px 4px 8px");
    expect(ruleTextAt(controlsWrap)).toContain("background: var(--bg)");
    expect(ruleTextAt(controlsWrap)).toContain("z-index: 2");
  });

  it("聊天滚动条默认隐藏，滚动或进入滚动条区域时再显示", () => {
    const transcript = selectorIndex(".chat-transcript {");
    const hiddenThumb = selectorIndex(".chat-transcript::-webkit-scrollbar-thumb");
    const visibleThumb = selectorIndex(".chat-transcript.is-scrollbar-visible::-webkit-scrollbar-thumb");
    const hiddenFirefox = selectorIndex(".chat-transcript:not(.is-scrollbar-visible)");
    const visibleFirefox = selectorIndex(".chat-transcript.is-scrollbar-visible");

    expect(transcript).toBeGreaterThan(-1);
    expect(hiddenThumb).toBeGreaterThan(transcript);
    expect(visibleThumb).toBeGreaterThan(hiddenThumb);
    expect(ruleTextAt(transcript)).toContain("--chat-scrollbar-transition-duration: 0.48s");
    expect(ruleTextAt(transcript)).toContain("transition: scrollbar-color var(--chat-scrollbar-transition-duration) ease");
    expect(ruleTextAt(transcript)).toContain("scrollbar-color: transparent transparent");
    expect(ruleTextAt(hiddenThumb)).toContain("background-color: transparent");
    expect(ruleTextAt(hiddenThumb)).toContain("transition: background-color var(--chat-scrollbar-transition-duration) ease");
    expect(ruleTextAt(visibleThumb)).toContain("background-color: var(--border-strong)");
    expect(ruleTextAt(hiddenFirefox)).toContain("scrollbar-color: transparent transparent");
    expect(ruleTextAt(visibleFirefox)).toContain("scrollbar-color: var(--border-strong) transparent");
    expect(chatTranscript).toContain("is-scrollbar-visible");
    expect(chatTranscript).toContain("@scrollend=\"onScrollEnd\"");
    expect(chatTranscript).toContain("@mousemove=\"onMouseMove\"");
    expect(chatTranscript).toContain("@mouseleave=\"onMouseLeave\"");
    expect(chatTranscript).toContain("const SCROLLBAR_HIDE_DELAY = 180");
    expect(chatTranscript).toContain("setTimeout");
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

  it("reasoning 折叠态给箭头保留空间，展开态箭头跟随完整文本", () => {
    const collapsedTitle = selectorIndex(".agent-timeline__item--reasoning.is-compact .agent-timeline__title");
    const collapsedText = selectorIndex(".agent-timeline__item--reasoning.is-compact .agent-timeline__title span");
    const baseText = selectorLastIndex(".agent-timeline__title span {");
    const expandedTitle = selectorIndex(".agent-timeline__item--reasoning:not(.is-compact) .agent-timeline__title");
    const expandedText = selectorIndex(".agent-timeline__item--reasoning:not(.is-compact) .agent-timeline__title span");
    const collapsedTitleRule = ruleTextAt(collapsedTitle);
    const collapsedTextRule = ruleTextAt(collapsedText);
    const baseTextRule = ruleTextAt(baseText);
    const expandedTitleRule = ruleTextAt(expandedTitle);
    const expandedTextRule = ruleTextAt(expandedText);

    expect(collapsedTitle).toBeGreaterThan(-1);
    expect(collapsedText).toBeGreaterThan(collapsedTitle);
    expect(baseText).toBeGreaterThan(collapsedText);
    expect(expandedTitle).toBeGreaterThan(collapsedText);
    expect(expandedText).toBeGreaterThan(expandedTitle);
    expect(collapsedTitleRule).toContain("width: 100%");
    expect(collapsedTitleRule).toContain("display: flex");
    expect(collapsedTitleRule).toContain("justify-content: space-between");
    expect(collapsedTextRule).toContain("min-width: 0");
    expect(collapsedTextRule).toContain("overflow: hidden");
    expect(collapsedTextRule).toContain("text-overflow: ellipsis");
    expect(baseTextRule).toContain("white-space: nowrap");
    expect(expandedTitleRule).toContain("display: inline-block");
    expect(expandedTitleRule).toContain("box-sizing: border-box");
    expect(expandedTitleRule).toContain("min-height: 22px");
    expect(expandedTitleRule).toContain("padding-top: calc((22px - 1.4em) / 2)");
    expect(expandedTitleRule).not.toContain("align-items: flex-start");
    expect(expandedTitleRule).not.toContain("justify-content: flex-start");
    expect(expandedTextRule).toContain("white-space: pre-wrap");
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
