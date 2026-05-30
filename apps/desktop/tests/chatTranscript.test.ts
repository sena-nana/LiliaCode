import { fireEvent, render } from "@testing-library/vue";
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

describe("ChatTranscript scrollbar visibility", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("滚动时显示滚动条，并在滚动结束后短暂延时再隐藏", async () => {
    const view = render(ChatTranscript, {
      props: {
        timelineEvents: [],
        emptyHeadline: "今天想做什么？",
        isThinking: false,
      },
    });
    const transcript = transcriptElement(view.container);

    expect(transcript).not.toHaveClass("is-scrollbar-visible");

    await fireEvent.scroll(transcript);

    expect(transcript).toHaveClass("is-scrollbar-visible");
    await fireEvent(transcript, new Event("scrollend"));
    await vi.advanceTimersByTimeAsync(179);
    expect(transcript).toHaveClass("is-scrollbar-visible");
    await vi.advanceTimersByTimeAsync(1);
    expect(transcript).not.toHaveClass("is-scrollbar-visible");
  });

  it("鼠标移动到滚动条区域时显示，离开区域后短暂延时再隐藏", async () => {
    const view = render(ChatTranscript, {
      props: {
        timelineEvents: [],
        emptyHeadline: "今天想做什么？",
        isThinking: false,
      },
    });
    const transcript = transcriptElement(view.container);
    mockTranscriptRect(transcript);

    await fireEvent.mouseMove(transcript, { clientX: 92, clientY: 20 });

    expect(transcript).toHaveClass("is-scrollbar-visible");

    await fireEvent.mouseMove(transcript, { clientX: 40, clientY: 20 });
    await vi.advanceTimersByTimeAsync(179);
    expect(transcript).toHaveClass("is-scrollbar-visible");
    await vi.advanceTimersByTimeAsync(1);
    expect(transcript).not.toHaveClass("is-scrollbar-visible");
  });

  it("滚动地图 thumb 精确覆盖未被输入区遮挡的可见对话区域", async () => {
    const events = [
      timelineEvent({
        id: "user-1",
        kind: "message",
        payload: { role: "user", content: "开始" },
        intraTurnOrder: 1,
      }),
      timelineEvent({
        id: "plan-1",
        kind: "plan",
        payload: { plan: "实现计划" },
        createdAt: 2,
        updatedAt: 2,
        intraTurnOrder: 2,
      }),
      timelineEvent({
        id: "command-1",
        kind: "command",
        payload: { command: "pwd" },
        createdAt: 3,
        updatedAt: 3,
        intraTurnOrder: 3,
      }),
      timelineEvent({
        id: "error-1",
        kind: "error",
        status: "error",
        payload: { message: "boom" },
        createdAt: 4,
        updatedAt: 4,
        intraTurnOrder: 4,
      }),
    ];
    const view = render(ChatTranscript, {
      props: {
        timelineEvents: events,
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
    mockElementRect(
      view.container.querySelector('[data-scroll-anchor-id="user-1"]') as HTMLElement,
      { top: 20, bottom: 50, height: 30 },
    );
    mockElementRect(
      view.container.querySelector('[data-scroll-anchor-id="plan-1"]') as HTMLElement,
      { top: 300, bottom: 322, height: 22 },
    );
    mockElementRect(
      view.container.querySelector('[data-scroll-anchor-id="command-1"]') as HTMLElement,
      { top: 420, bottom: 442, height: 22 },
    );
    mockElementRect(
      view.container.querySelector('[data-scroll-anchor-id="error-1"]') as HTMLElement,
      { top: 600, bottom: 622, height: 22 },
    );

    await fireEvent.scroll(transcript);
    await flushScrollMapFrame();

    const thumb = view.container.querySelector(".chat-scroll-map__thumb") as HTMLElement;
    expect(thumb).toBeInstanceOf(HTMLElement);
    expect(parseFloat(thumb.style.height)).toBeCloseTo(31.4, 1);
    expect(thumb.style.transform).toContain("17.446");
    expect((view.container.querySelector(".chat-scroll-map") as HTMLElement).style.getPropertyValue(
      "--chat-scroll-map-bottom-offset",
    )).toBe("60px");
    expect(view.container.querySelectorAll(".chat-scroll-map__marker")).toHaveLength(3);
    expect(view.container.querySelector(".chat-scroll-map__marker--user")).toBeInTheDocument();
    expect(view.container.querySelector(".chat-scroll-map__marker--plan")).toBeInTheDocument();
    expect(view.container.querySelector(".chat-scroll-map__marker--error")).toBeInTheDocument();
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

  it("新滚动条支持轨道翻页和 thumb 拖拽", async () => {
    let transcript: HTMLElement;
    const scrollTo = vi.fn(({ top }: ScrollToOptions) => {
      transcript.scrollTop = Number(top);
    });
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
    transcript = transcriptElement(view.container);
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

    const track = view.container.querySelector(".chat-scroll-map__track") as HTMLElement;
    const thumb = view.container.querySelector(".chat-scroll-map__thumb") as HTMLElement;
    mockElementRect(track, { top: 0, bottom: 164, height: 164 });

    await fireEvent.pointerDown(track, { clientY: 120, pointerId: 1 });
    expect(scrollTo).toHaveBeenLastCalledWith({
      top: 280,
      behavior: "auto",
    });

    await fireEvent.pointerDown(thumb, { clientY: 20, pointerId: 2 });
    await fireEvent.pointerMove(window, { clientY: 61, pointerId: 2 });
    expect(transcript.scrollTop).toBeCloseTo(515, 0);
    await fireEvent.pointerUp(window, { clientY: 61, pointerId: 2 });
  });
});
