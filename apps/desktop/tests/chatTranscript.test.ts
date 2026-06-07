import { fireEvent, render } from "@testing-library/vue";
import { nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentTimelineEvent } from "@lilia/contracts";
import ChatTranscript from "../src/components/chat/ChatTranscript.vue";

function transcriptElement(container: HTMLElement): HTMLElement {
  const element = container.querySelector(".chat-transcript");
  if (!(element instanceof HTMLElement)) {
    throw new Error("chat transcript element not found");
  }
  return element;
}

function scrollMapElement(container: HTMLElement): HTMLElement {
  const element = container.querySelector(".chat-scroll-map");
  if (!(element instanceof HTMLElement)) {
    throw new Error("chat scroll map element not found");
  }
  return element;
}

function mockTranscriptRect(element: HTMLElement) {
  vi.spyOn(element, "getBoundingClientRect").mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 100,
    bottom: 240,
    width: 100,
    height: 240,
    toJSON: () => ({}),
  });
}

function mockElementRect(element: HTMLElement, rect: Partial<DOMRect>) {
  vi.spyOn(element, "getBoundingClientRect").mockReturnValue({
    x: rect.left ?? 0,
    y: rect.top ?? 0,
    left: rect.left ?? 0,
    top: rect.top ?? 0,
    right: rect.right ?? 100,
    bottom: rect.bottom ?? 0,
    width: rect.width ?? 100,
    height: rect.height ?? 0,
    toJSON: () => ({}),
  });
}

function mockScrollGeometry(
  element: HTMLElement,
  values: {
    clientHeight: number;
    scrollHeight: number;
    scrollTop: number;
  },
) {
  Object.defineProperty(element, "clientHeight", {
    configurable: true,
    value: values.clientHeight,
  });
  Object.defineProperty(element, "scrollHeight", {
    configurable: true,
    value: values.scrollHeight,
  });
  element.scrollTop = values.scrollTop;
}

function renderTranscriptWithControls(timelineEvents: AgentTimelineEvent[]) {
  const view = render(ChatTranscript, {
    props: {
      timelineEvents,
      emptyHeadline: "今天想做什么？",
      isThinking: false,
    },
    slots: {
      controls: "<div data-testid=\"composer\">composer</div>",
    },
  });
  const transcript = transcriptElement(view.container);
  const controls = view.container.querySelector(".chat-controls-wrap");
  expect(controls).toBeInstanceOf(HTMLElement);
  return { controls: controls as HTMLElement, transcript, view };
}

function mockTranscriptViewport(
  transcript: HTMLElement,
  controls: HTMLElement,
  scrollTo?: ReturnType<typeof vi.fn>,
) {
  if (scrollTo) {
    Object.defineProperty(transcript, "scrollTo", {
      configurable: true,
      value: scrollTo,
    });
  }
  mockScrollGeometry(transcript, {
    clientHeight: 240,
    scrollHeight: 1000,
    scrollTop: 100,
  });
  mockTranscriptRect(transcript);
  mockElementRect(controls, {
    top: 180,
    bottom: 240,
    height: 60,
  });
}

function mockScrollMapGeometry(
  view: { container: HTMLElement },
  transcript: HTMLElement,
  controls: HTMLElement,
  anchorRects: Record<string, Partial<DOMRect>>,
) {
  mockTranscriptViewport(transcript, controls);
  for (const [id, rect] of Object.entries(anchorRects)) {
    const anchor = view.container.querySelector(`[data-scroll-anchor-id="${id}"]`);
    expect(anchor).toBeInstanceOf(HTMLElement);
    mockElementRect(anchor as HTMLElement, rect);
  }
}

function mockPlanRevealGeometry(
  view: { container: HTMLElement },
  transcript: HTMLElement,
  controls: HTMLElement,
  scrollTo: ReturnType<typeof vi.fn>,
  cardRect: Partial<DOMRect>,
) {
  const card = view.container.querySelector(".timeline-card--plan");
  expect(card).toBeInstanceOf(HTMLElement);
  mockTranscriptViewport(transcript, controls, scrollTo);
  mockElementRect(card as HTMLElement, cardRect);
}

function tooltipText(marker: HTMLElement): string {
  const tooltip = marker.querySelector('[role="tooltip"]');
  expect(tooltip).toBeInstanceOf(HTMLElement);
  return tooltip?.textContent ?? "";
}

function timelineEvent(
  patch: Partial<AgentTimelineEvent> & Pick<AgentTimelineEvent, "id" | "kind" | "payload">,
): AgentTimelineEvent {
  return {
    id: patch.id,
    taskId: "task-1",
    turnId: null,
    backend: "claude",
    kind: patch.kind,
    status: patch.status ?? "success",
    title: patch.title ?? patch.kind,
    summary: patch.summary ?? "",
    payload: patch.payload,
    createdAt: patch.createdAt ?? 1,
    updatedAt: patch.updatedAt ?? patch.createdAt ?? 1,
    turnSeq: patch.turnSeq ?? 1,
    intraTurnOrder: patch.intraTurnOrder ?? 1,
  };
}

async function flushScrollMapFrame() {
  await vi.advanceTimersByTimeAsync(20);
  await Promise.resolve();
}

async function flushTranscriptScroll() {
  await nextTick();
  await nextTick();
}

describe("ChatTranscript scrollbar visibility", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });


  afterEach(() => {
    vi.useRealTimers();
  });

  it("滚动时显示滚动条，并在滚动结束后短暂延时再隐藏", async () => {
    const { controls, transcript, view } = renderTranscriptWithControls([]);
    mockTranscriptViewport(transcript, controls);
    await flushScrollMapFrame();

    expect(scrollMapElement(view.container)).not.toHaveClass("is-visible");

    await fireEvent.scroll(transcript);
    await flushScrollMapFrame();

    expect(scrollMapElement(view.container)).toHaveClass("is-visible");
    await fireEvent(transcript, new Event("scrollend"));
    await vi.advanceTimersByTimeAsync(179);
    expect(scrollMapElement(view.container)).toHaveClass("is-visible");
    await vi.advanceTimersByTimeAsync(1);
    expect(scrollMapElement(view.container)).not.toHaveClass("is-visible");
  });

  it("用户发送触发强制滚动时，即使当前不贴底也会滚到底部并恢复跟随", async () => {
    const events = [
      timelineEvent({
        id: "user-1",
        kind: "message",
        payload: { role: "user", content: "第一条消息" },
      }),
    ];
    const view = render(ChatTranscript, {
      props: {
        timelineEvents: events,
        emptyHeadline: "今天想做什么？",
        isThinking: false,
        forceScrollBottomKey: 0,
      },
    });
    const transcript = transcriptElement(view.container);
    mockScrollGeometry(transcript, {
      clientHeight: 240,
      scrollHeight: 1000,
      scrollTop: 100,
    });

    await fireEvent.scroll(transcript);
    await view.rerender({
      timelineEvents: events,
      emptyHeadline: "今天想做什么？",
      isThinking: false,
      forceScrollBottomKey: 1,
    });
    await flushTranscriptScroll();

    expect(transcript.scrollTop).toBe(1000);

    mockScrollGeometry(transcript, {
      clientHeight: 240,
      scrollHeight: 1200,
      scrollTop: transcript.scrollTop,
    });
    await view.rerender({
      timelineEvents: [
        ...events,
        timelineEvent({
          id: "assistant-1",
          kind: "message",
          payload: { role: "assistant", content: "跟随回复" },
          intraTurnOrder: 2,
        }),
      ],
      emptyHeadline: "今天想做什么？",
      isThinking: false,
      forceScrollBottomKey: 1,
    });
    await flushTranscriptScroll();

    expect(transcript.scrollTop).toBe(1200);
  });

  it("点击滚动地图 marker 跳到对应节点", async () => {
    const scrollTo = vi.fn();
    const view = render(ChatTranscript, {
      props: {
        timelineEvents: [
          timelineEvent({
            id: "plan-1",
            kind: "plan",
            payload: { plan: "实现计划" },
          }),
        ],
        emptyHeadline: "今天想做什么？",
        isThinking: false,
      },
      slots: {
        controls: "<div data-testid=\"composer\">composer</div>",
      },
    });
    const transcript = transcriptElement(view.container);
    const controls = view.container.querySelector(".chat-controls-wrap");
    const planAnchor = view.container.querySelector('[data-scroll-anchor-id="plan-1"]');

    expect(controls).toBeInstanceOf(HTMLElement);
    expect(planAnchor).toBeInstanceOf(HTMLElement);

    Object.defineProperty(transcript, "scrollTo", {
      configurable: true,
      value: scrollTo,
    });
    mockScrollGeometry(transcript, {
      clientHeight: 240,
      scrollHeight: 1000,
      scrollTop: 100,
    });
    mockTranscriptRect(transcript);
    mockElementRect(controls as HTMLElement, {
      top: 180,
      bottom: 240,
      height: 60,
    });
    mockElementRect(planAnchor as HTMLElement, {
      top: 300,
      bottom: 322,
      height: 22,
    });

    await fireEvent.scroll(transcript);
    await flushScrollMapFrame();
    await fireEvent.click(view.getByRole("button", { name: "跳到计划位置" }));

    expect(scrollTo).toHaveBeenCalledWith({
      top: 384,
      behavior: "smooth",
    });
  });
});

describe("ChatTranscript agent selection toolbar", () => {
  it("拖选 Agent 回复后显示工具栏并可引用", async () => {
    const view = render(ChatTranscript, {
      props: {
        timelineEvents: [
          timelineEvent({
            id: "assistant-1",
            kind: "message",
            payload: { role: "assistant", content: "这是一段可操作文本" },
          }),
        ],
        emptyHeadline: "今天想做什么？",
      },
    });
    const walker = document.createTreeWalker(view.container, NodeFilter.SHOW_TEXT);
    let textNode = walker.nextNode() as Text | null;
    while (textNode && !textNode.textContent?.includes("可操作文本")) {
      textNode = walker.nextNode() as Text | null;
    }
    expect(textNode?.textContent).toContain("可操作文本");
    const range = document.createRange();
    const start = textNode!.textContent!.indexOf("可操作文本");
    range.setStart(textNode!, start);
    range.setEnd(textNode!, start + "可操作文本".length);
    Object.defineProperty(range, "getBoundingClientRect", {
      configurable: true,
      value: vi.fn(() => ({
        x: 40,
        y: 80,
        left: 40,
        top: 80,
        right: 160,
        bottom: 100,
        width: 120,
        height: 20,
        toJSON: () => ({}),
      })),
    });
    mockElementRect(view.container.querySelector(".chat-transcript-frame") as HTMLElement, {
      left: 0,
      top: 0,
      right: 400,
      bottom: 300,
      width: 400,
      height: 300,
    });

    const selectable = view.container.querySelector("[data-agent-selectable='true']");
    expect(selectable).toBeInstanceOf(HTMLElement);
    (selectable as HTMLElement).dispatchEvent(new PointerEvent("pointerdown", {
      bubbles: true,
      composed: true,
    }));
    window.getSelection()?.removeAllRanges();
    window.getSelection()?.addRange(range);
    document.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
    await nextTick();
    await nextTick();

    expect(view.getByRole("toolbar", { name: "Agent 选中文本操作" })).toBeInTheDocument();
    expect(view.getByRole("button", { name: "复制" })).toHaveAttribute("title", "复制");
    await fireEvent.click(view.getByRole("button", { name: "引用" }));
    expect(view.emitted("insert-draft-text")).toEqual([["> 可操作文本\n\n"]]);
  });
});
