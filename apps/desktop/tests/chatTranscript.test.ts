import { fireEvent, render } from "@testing-library/vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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
});
