import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const styles = readFileSync("src/styles.css", "utf8");
const baseScrollMap = readFileSync("src/components/chat/BaseScrollMap.vue", "utf8");
const chatTranscript = readFileSync("src/components/chat/ChatTranscript.vue", "utf8");
const chatScrollMap = readFileSync("src/components/chat/ChatScrollMap.vue", "utf8");
const timelineDeclaredEvent = readFileSync("src/components/chat/TimelineDeclaredEvent.vue", "utf8");
const timelineCardDetails = readFileSync("src/components/chat/TimelineCardDetails.vue", "utf8");
const timelinePlanCard = readFileSync("src/components/chat/TimelinePlanCard.vue", "utf8");
const useScrollMap = readFileSync("src/components/chat/useScrollMap.ts", "utf8");
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
    expect(chatTranscript).toContain('ref="frameEl"');
    expect(chatTranscript).toContain(":hover-target=\"frameEl\"");
    expect(chatTranscript).not.toContain("is-scrollbar-visible");
    expect(chatTranscript).not.toContain("@scrollend=\"onScrollEnd\"");
    expect(chatTranscript).not.toContain("@mousemove=\"onMouseMove\"");
    expect(chatTranscript).not.toContain("@mouseleave=\"onMouseLeave\"");
    expect(chatTranscript).not.toContain("useScrollMapVisibility");
    expect(baseScrollMap).toContain("useScrollMapVisibility");
    expect(useScrollMap).toContain("addEventListener(\"scrollend\", onScrollEnd)");
    expect(useScrollMap).toContain("addEventListener(\"mousemove\", onMouseMove)");
    expect(useScrollMap).toContain("addEventListener(\"mouseleave\", onMouseLeave)");
    expect(useScrollMap).toContain("const DEFAULT_HIDE_DELAY = 180");
    expect(useScrollMap).toContain("setTimeout");
  });

  it("滚动地图作为覆盖层显示视口 thumb 和关键节点 marker", () => {
    const frame = selectorIndex(".chat-transcript-frame {");
    const transcript = selectorIndex(".chat-transcript {");
    const baseMap = selectorIndex(".scroll-map {");
    const visibleScrollMap = selectorIndex(".scroll-map.is-visible,");
    const track = selectorIndex(".scroll-map__track {");
    const thumb = selectorIndex(".scroll-map__thumb {");
    const thumbVisible = selectorIndex(".scroll-map__thumb::before");
    const activeThumb = selectorIndex(".scroll-map__track:hover .scroll-map__thumb::before,");
    const scrollMap = selectorIndex(".chat-scroll-map {");
    const marker = selectorIndex(".chat-scroll-map__marker {");
    const markerBefore = selectorIndex(".chat-scroll-map__marker::before");
    const planMarker = selectorIndex(".chat-scroll-map__marker--plan");
    const errorMarker = selectorIndex(".chat-scroll-map__marker--error");
    const tooltip = selectorIndex(".chat-scroll-map__tooltip {");
    const visibleTooltip = selectorIndex(".chat-scroll-map__marker:hover .chat-scroll-map__tooltip");
    const tooltipItem = selectorIndex(".chat-scroll-map__tooltip-item {");
    const tooltipTitle = selectorIndex(".chat-scroll-map__tooltip-title {");
    const tooltipSummary = selectorIndex(".chat-scroll-map__tooltip-summary {");

    expect(frame).toBeGreaterThan(-1);
    expect(transcript).toBeGreaterThan(frame);
    expect(baseMap).toBeGreaterThan(transcript);
    expect(visibleScrollMap).toBeGreaterThan(baseMap);
    expect(track).toBeGreaterThan(visibleScrollMap);
    expect(thumb).toBeGreaterThan(track);
    expect(thumbVisible).toBeGreaterThan(thumb);
    expect(activeThumb).toBeGreaterThan(thumbVisible);
    expect(scrollMap).toBeGreaterThan(activeThumb);
    expect(marker).toBeGreaterThan(scrollMap);
    expect(markerBefore).toBeGreaterThan(marker);
    expect(planMarker).toBeGreaterThan(markerBefore);
    expect(errorMarker).toBeGreaterThan(planMarker);
    expect(tooltip).toBeGreaterThan(errorMarker);
    expect(visibleTooltip).toBeGreaterThan(tooltip);
    expect(tooltipItem).toBeGreaterThan(visibleTooltip);
    expect(tooltipTitle).toBeGreaterThan(tooltipItem);
    expect(tooltipSummary).toBeGreaterThan(tooltipTitle);
    expect(ruleTextAt(frame)).toContain("position: relative");
    expect(ruleTextAt(frame)).toContain("--chat-scrollbar-transition-duration: 0.48s");
    expect(ruleTextAt(transcript)).toContain("scrollbar-gutter: stable");
    expect(ruleTextAt(baseMap)).toContain("width: 10px");
    expect(ruleTextAt(baseMap)).toContain("opacity: 0");
    expect(ruleTextAt(baseMap)).toContain("pointer-events: none");
    expect(ruleTextAt(baseMap)).toContain("transition: opacity var(--chat-scrollbar-transition-duration, 0.48s) ease");
    expect(ruleTextAt(scrollMap)).toContain("position: absolute");
    expect(ruleTextAt(scrollMap)).toContain("bottom: calc(8px + var(--chat-scroll-map-bottom-offset))");
    expect(ruleTextAt(visibleScrollMap)).toContain("opacity: 1");
    expect(ruleTextAt(visibleScrollMap)).toContain("pointer-events: auto");
    expect(ruleTextAt(track)).toContain("cursor: default");
    expect(ruleTextAt(thumb)).toContain("right: 0");
    expect(ruleTextAt(thumb)).toContain("width: 100%");
    expect(ruleTextAt(thumb)).not.toContain("cursor: ns-resize");
    expect(ruleTextAt(thumbVisible)).toContain("right: 3px");
    expect(ruleTextAt(thumbVisible)).toContain("width: 4px");
    expect(ruleTextAt(thumbVisible)).toContain("background: var(--border-strong)");
    expect(ruleTextAt(activeThumb)).toContain(".scroll-map.is-dragging .scroll-map__thumb::before");
    expect(ruleTextAt(activeThumb)).toContain("background: var(--text-faint)");
    expect(ruleTextAt(marker)).toContain("pointer-events: auto");
    expect(ruleTextAt(marker)).toContain("color: var(--accent)");
    expect(ruleTextAt(planMarker)).toContain("color: var(--warn)");
    expect(ruleTextAt(errorMarker)).toContain("color: var(--err)");
    expect(ruleTextAt(tooltip)).toContain("right: 14px");
    expect(ruleTextAt(tooltip)).toContain("max-width: min(320px, calc(100vw - 48px))");
    expect(ruleTextAt(tooltip)).toContain("background: var(--bg-elev)");
    expect(ruleTextAt(tooltip)).toContain("border: 1px solid var(--border)");
    expect(ruleTextAt(tooltip)).toContain("visibility: hidden");
    expect(ruleTextAt(tooltip)).toContain("pointer-events: none");
    expect(ruleTextAt(visibleTooltip)).toContain("opacity: 1");
    expect(ruleTextAt(visibleTooltip)).toContain("visibility: visible");
    expect(ruleTextAt(tooltipTitle)).toContain("-webkit-line-clamp: 1");
    expect(ruleTextAt(tooltipSummary)).toContain("-webkit-line-clamp: 2");
    expect(chatTranscript).toContain("<ChatScrollMap");
    expect(chatTranscript).toContain(":project-cwd=\"projectCwd\"");
    expect(chatScrollMap).toContain("<BaseScrollMap");
    expect(baseScrollMap).toContain("class=\"scroll-map\"");
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

  it("计划结果使用灰色卡片，折叠态仍保留摘要和卡片外观", () => {
    const planCard = selectorIndex(".timeline-card--plan {");
    const planNode = selectorIndex(".agent-timeline__item--plan .agent-timeline__node");
    const planHead = selectorIndex(".timeline-plan-card__head {");
    const planPendingBadge = selectorIndex(".timeline-plan-card__badge--pending,");
    const planApprovedBadge = selectorIndex(".timeline-plan-card__badge--approved {");
    const planSummary = selectorIndex(".timeline-plan-card__summary {");
    const planBodyShell = selectorIndex(".timeline-plan-card__body-shell {");
    const planBody = selectorIndex(".timeline-plan-card__body {");
    const planContent = selectorIndex(".timeline-plan-card__content {");
    const planMarkdownHeading4 = selectorIndex(".timeline-plan-card__markdown--plan h4.markdown-block__heading {");
    const planMarkdownHeading5 = selectorIndex(".timeline-plan-card__markdown--plan h5.markdown-block__heading {");
    const planMarkdownHeading6 = selectorIndex(".timeline-plan-card__markdown--plan h6.markdown-block__heading {");
    const planRevision = selectorIndex(".timeline-plan-card__section--revision {");
    const planPromptList = selectorIndex(".timeline-plan-card__prompt-list {");

    expect(planCard).toBeGreaterThan(-1);
    expect(planNode).toBeGreaterThan(planCard);
    expect(planHead).toBeGreaterThan(planNode);
    expect(planPendingBadge).toBeGreaterThan(planHead);
    expect(planApprovedBadge).toBeGreaterThan(planPendingBadge);
    expect(planSummary).toBeGreaterThan(planApprovedBadge);
    expect(planBodyShell).toBeGreaterThan(planSummary);
    expect(planBody).toBeGreaterThan(planBodyShell);
    expect(planContent).toBeGreaterThan(planBody);
    expect(planMarkdownHeading4).toBeGreaterThan(planContent);
    expect(planMarkdownHeading5).toBeGreaterThan(planMarkdownHeading4);
    expect(planMarkdownHeading6).toBeGreaterThan(planMarkdownHeading5);
    expect(planRevision).toBeGreaterThan(planMarkdownHeading6);
    expect(planPromptList).toBeGreaterThan(planRevision);
    expect(ruleTextAt(planCard)).toContain("background: var(--bg-subtle)");
    expect(ruleTextAt(planCard)).toContain("border: 1px solid var(--border-soft)");
    expect(ruleTextAt(planCard)).not.toContain("border-left");
    expect(ruleTextAt(planCard)).toContain("border-radius: 6px");
    expect(ruleTextAt(planNode)).toContain("background: transparent");
    expect(ruleTextAt(planHead)).toContain("padding: 8px 9px");
    expect(ruleTextAt(planPendingBadge)).toContain("background: var(--warn-soft)");
    expect(ruleTextAt(planPendingBadge)).toContain("color: var(--warn)");
    expect(ruleTextAt(planApprovedBadge)).toContain("background: var(--ok-soft)");
    expect(ruleTextAt(planApprovedBadge)).toContain("color: var(--ok)");
    expect(ruleTextAt(planSummary)).toContain("text-overflow: ellipsis");
    expect(ruleTextAt(planSummary)).toContain("white-space: nowrap");
    expect(ruleTextAt(planBodyShell)).toContain("border-top: 1px solid var(--border-soft)");
    expect(ruleTextAt(planBody)).toContain("max-height: min(52vh, 560px)");
    expect(ruleTextAt(planBody)).toContain("overflow-y: auto");
    expect(ruleTextAt(planContent)).toContain("gap: 8px");
    expect(ruleTextAt(planMarkdownHeading4)).toContain("color: var(--accent)");
    expect(ruleTextAt(planMarkdownHeading5)).toContain("color: var(--warn)");
    expect(ruleTextAt(planMarkdownHeading6)).toContain("color: var(--ok)");
    expect(ruleTextAt(planRevision)).toContain("border-left: 2px solid");
    expect(ruleTextAt(planPromptList)).toContain("list-style: none");
    expect(timelinePlanCard).toContain('class="timeline-card timeline-card--plan"');
    expect(timelinePlanCard).toContain(":aria-expanded=\"expanded\"");
    expect(timelinePlanCard).toContain('class="timeline-plan-card__body-shell"');
    expect(timelinePlanCard).not.toContain("<BaseScrollMap");
    expect(timelinePlanCard).not.toContain("timeline-plan-card__heading-marker");
    expect(timelinePlanCard).not.toContain("跳到计划标题");
    expect(timelinePlanCard).toContain('class="timeline-plan-card__content"');
    expect(timelinePlanCard).toContain("timeline-plan-card__markdown--plan");
    expect(timelinePlanCard).toContain("timeline-plan-card__badge--${statusKind}");
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

    expect(timelineCardDetails).toContain('class="timeline-card__field-list"');
    expect(timelineCardDetails).toContain('class="timeline-card__section timeline-card__section--code"');
    expect(timelineCardDetails).toContain('class="timeline-card__section timeline-card__section--list"');
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
