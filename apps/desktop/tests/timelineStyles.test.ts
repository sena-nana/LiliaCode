import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const styles = readFileSync("src/styles.css", "utf8");
const chatTranscript = readFileSync("src/components/chat/ChatTranscript.vue", "utf8");
const timelineDeclaredEvent = readFileSync("src/components/chat/TimelineDeclaredEvent.vue", "utf8");
const taskDetail = readFileSync("src/pages/TaskDetail.vue", "utf8");

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

  it("聊天原生滚动条隐藏，滚动或进入热区时改由滚动地图显示", () => {
    const transcript = selectorIndex(".chat-transcript {");
    const nativeScrollbar = selectorIndex(".chat-transcript::-webkit-scrollbar {");
    const oldVisibleThumb = selectorIndex(".chat-transcript.is-scrollbar-visible::-webkit-scrollbar-thumb");

    expect(transcript).toBeGreaterThan(-1);
    expect(nativeScrollbar).toBeGreaterThan(transcript);
    expect(oldVisibleThumb).toBe(-1);
    expect(ruleTextAt(transcript)).toContain("scrollbar-width: none");
    expect(ruleTextAt(transcript)).not.toContain("scrollbar-color");
    expect(ruleTextAt(transcript)).not.toContain("scrollbar-color var(");
    expect(ruleTextAt(nativeScrollbar)).toContain("width: 0");
    expect(ruleTextAt(nativeScrollbar)).toContain("height: 0");
    expect(chatTranscript).toContain("is-scrollbar-visible");
    expect(chatTranscript).toContain("@scrollend=\"onScrollEnd\"");
    expect(chatTranscript).toContain("@mousemove=\"onMouseMove\"");
    expect(chatTranscript).toContain("@mouseleave=\"onMouseLeave\"");
    expect(chatTranscript).toContain("const SCROLLBAR_HIDE_DELAY = 180");
    expect(chatTranscript).toContain("setTimeout");
  });

  it("滚动地图作为覆盖层显示视口 thumb 和关键节点 marker", () => {
    const frame = selectorIndex(".chat-transcript-frame {");
    const transcript = selectorIndex(".chat-transcript {");
    const scrollMap = selectorIndex(".chat-scroll-map {");
    const visibleScrollMap = selectorIndex(".chat-scroll-map.is-visible");
    const track = selectorIndex(".chat-scroll-map__track {");
    const thumb = selectorIndex(".chat-scroll-map__thumb {");
    const draggingThumb = selectorIndex(".chat-scroll-map.is-dragging .chat-scroll-map__thumb");
    const marker = selectorIndex(".chat-scroll-map__marker {");
    const markerBefore = selectorIndex(".chat-scroll-map__marker::before");
    const planMarker = selectorIndex(".chat-scroll-map__marker--plan");
    const errorMarker = selectorIndex(".chat-scroll-map__marker--error");

    expect(frame).toBeGreaterThan(-1);
    expect(transcript).toBeGreaterThan(frame);
    expect(scrollMap).toBeGreaterThan(transcript);
    expect(visibleScrollMap).toBeGreaterThan(scrollMap);
    expect(track).toBeGreaterThan(visibleScrollMap);
    expect(thumb).toBeGreaterThan(track);
    expect(draggingThumb).toBeGreaterThan(thumb);
    expect(marker).toBeGreaterThan(draggingThumb);
    expect(markerBefore).toBeGreaterThan(marker);
    expect(planMarker).toBeGreaterThan(markerBefore);
    expect(errorMarker).toBeGreaterThan(planMarker);
    expect(ruleTextAt(frame)).toContain("position: relative");
    expect(ruleTextAt(frame)).toContain("--chat-scrollbar-transition-duration: 0.48s");
    expect(ruleTextAt(transcript)).toContain("scrollbar-gutter: stable");
    expect(ruleTextAt(scrollMap)).toContain("position: absolute");
    expect(ruleTextAt(scrollMap)).toContain("bottom: calc(8px + var(--chat-scroll-map-bottom-offset))");
    expect(ruleTextAt(scrollMap)).toContain("opacity: 0");
    expect(ruleTextAt(scrollMap)).toContain("pointer-events: none");
    expect(ruleTextAt(scrollMap)).toContain("transition: opacity var(--chat-scrollbar-transition-duration) ease");
    expect(ruleTextAt(visibleScrollMap)).toContain("opacity: 1");
    expect(ruleTextAt(visibleScrollMap)).toContain("pointer-events: auto");
    expect(ruleTextAt(thumb)).toContain("background: var(--border-strong)");
    expect(ruleTextAt(thumb)).toContain("cursor: ns-resize");
    expect(ruleTextAt(thumb)).toContain("pointer-events: auto");
    expect(ruleTextAt(draggingThumb)).toContain("background: var(--text-faint)");
    expect(ruleTextAt(marker)).toContain("pointer-events: auto");
    expect(ruleTextAt(marker)).toContain("color: var(--accent)");
    expect(ruleTextAt(planMarker)).toContain("color: var(--warn)");
    expect(ruleTextAt(errorMarker)).toContain("color: var(--err)");
    expect(chatTranscript).toContain("<ChatScrollMap");
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

  it("失败折叠项 hover 时继续使用错误色，过程摘要没有失败色和横向分割条", () => {
    const titleHover = selectorIndex(".agent-timeline__title:hover:not(:disabled)");
    const failedTitleHover = selectorIndex(
      ".agent-timeline__item:is(.is-status-failed, .is-status-error, .is-status-cancelled) .agent-timeline__title:hover:not(:disabled)",
    );
    const processToggle = selectorIndex(".agent-timeline__process-toggle {");
    const processSummary = selectorIndex(".agent-timeline__process-summary");
    const processDivider = selectorIndex(".agent-timeline__process-toggle::after");
    const processHover = selectorIndex(".agent-timeline__process-toggle:hover");
    const failedProcess = selectorIndex(".agent-timeline__process-toggle--failed");

    expect(titleHover).toBeGreaterThan(-1);
    expect(processToggle).toBeGreaterThan(-1);
    expect(processHover).toBeGreaterThan(-1);
    expect(failedTitleHover).toBeGreaterThan(titleHover);
    expect(processSummary).toBeGreaterThan(processToggle);
    expect(processDivider).toBe(-1);
    expect(ruleTextAt(processToggle)).toContain("flex: 0 1 auto");
    expect(ruleTextAt(processToggle)).not.toContain("gap:");
    expect(ruleTextAt(processSummary)).toContain("overflow: hidden");
    expect(failedProcess).toBe(-1);
    expect(ruleTextAt(failedTitleHover)).toContain("color: var(--err)");
  });

  it("最终回复过程组使用侧边栏同款 grid 折叠动画", () => {
    const processCollapse = selectorIndex(".agent-timeline__process-collapse {");
    const processOpen = selectorIndex(".agent-timeline__process-collapse.is-open {");
    const processInner = selectorIndex(".agent-timeline__process-collapse-inner {");
    const processOpenInner = selectorIndex(
      ".agent-timeline__process-collapse.is-open .agent-timeline__process-collapse-inner",
    );
    const processItem = selectorIndex(".agent-timeline__process-collapse .agent-timeline__item {");
    const processFirstItem = selectorIndex(".agent-timeline__process-collapse .agent-timeline__item:first-child");

    expect(processCollapse).toBeGreaterThan(-1);
    expect(processOpen).toBeGreaterThan(processCollapse);
    expect(processInner).toBeGreaterThan(processOpen);
    expect(processOpenInner).toBeGreaterThan(processInner);
    expect(processItem).toBeGreaterThan(processOpenInner);
    expect(ruleTextAt(processCollapse)).toContain("display: grid");
    expect(ruleTextAt(processCollapse)).toContain("grid-template-rows: 0fr");
    expect(ruleTextAt(processCollapse)).toContain("margin-left: calc(-1 * var(--agent-timeline-rail-offset))");
    expect(ruleTextAt(processCollapse)).toContain("width: calc(100% + var(--agent-timeline-rail-offset))");
    expect(ruleTextAt(processCollapse)).toContain(
      "transition: grid-template-rows 0.26s cubic-bezier(0.65, 0, 0.35, 1)",
    );
    expect(ruleTextAt(processOpen)).toContain("grid-template-rows: 1fr");
    expect(ruleTextAt(processInner)).toContain("overflow: hidden");
    expect(ruleTextAt(processInner)).toContain("min-height: 0");
    expect(ruleTextAt(processInner)).toContain(
      "transition: opacity 0.2s cubic-bezier(0.65, 0, 0.35, 1)",
    );
    expect(ruleTextAt(processOpenInner)).toContain("opacity: 1");
    expect(ruleTextAt(processItem)).toContain("padding: 7px 0");
    expect(processFirstItem).toBe(-1);
  });

  it("工具展开详情使用紧凑层级，字段组不复用外层详情容器", () => {
    const details = selectorIndex(".timeline-card__details {");
    const fieldList = selectorIndex(".timeline-card__field-list {");
    const codeSection = selectorIndex(".timeline-card__section--code");
    const listSection = selectorIndex(".timeline-card__section--list");
    const field = selectorIndex(".timeline-card__field {");
    const codeBlock = selectorIndex(".timeline-code-block {");
    const codeBlockCode = selectorIndex(".timeline-code-block code {");

    expect(details).toBeGreaterThan(-1);
    expect(fieldList).toBeGreaterThan(details);
    expect(codeSection).toBeGreaterThan(details);
    expect(listSection).toBeGreaterThan(codeSection);
    expect(field).toBeGreaterThan(fieldList);
    expect(codeBlock).toBeGreaterThan(field);
    expect(codeBlockCode).toBeGreaterThan(codeBlock);

    expect(timelineDeclaredEvent).toContain('class="timeline-card__field-list"');
    expect(timelineDeclaredEvent).toContain('class="timeline-card__section timeline-card__section--code"');
    expect(timelineDeclaredEvent).toContain('class="timeline-card__section timeline-card__section--list"');
    expect(ruleTextAt(details)).toContain("gap: 6px");
    expect(ruleTextAt(fieldList)).toContain("gap: 3px");
    expect(ruleTextAt(fieldList)).toContain("padding-left: 8px");
    expect(ruleTextAt(fieldList)).toContain("border-left: 1px solid var(--border-soft)");
    expect(ruleTextAt(field)).toContain("line-height: 1.45");
    expect(ruleTextAt(codeBlock)).toContain("max-width: 100%");
    expect(ruleTextAt(codeBlock)).toContain("padding: 7px 8px");
    expect(ruleTextAt(codeBlock)).toContain("border-radius: 5px");
    expect(ruleTextAt(codeBlock)).toContain("line-height: 1.5");
    expect(ruleTextAt(codeBlock)).toContain("white-space: pre-wrap");
    expect(ruleTextAt(codeBlock)).toContain("overflow-wrap: anywhere");
    expect(ruleTextAt(codeBlockCode)).toContain("white-space: inherit");
  });

  it("组内工具展开内容与缩略行保持间距", () => {
    const groupDetailContent = selectorIndex(".agent-timeline__group-head + .agent-timeline__content");

    expect(groupDetailContent).toBeGreaterThan(-1);
    expect(ruleTextAt(groupDetailContent)).toContain("margin-top: 7px");
  });
});
