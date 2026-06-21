import { render } from "@testing-library/vue";
import { defineComponent, nextTick } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useTimelineRailMask } from "../src/components/chat/useTimelineRailMask";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("useTimelineRailMask", () => {
  it("卸载后忽略已进入 nextTick 阶段的 rail 测量", async () => {
    let rafCallback: FrameRequestCallback | null = null;
    vi.stubGlobal("requestAnimationFrame", vi.fn((callback: FrameRequestCallback) => {
      rafCallback = callback;
      return 1;
    }));
    vi.stubGlobal("cancelAnimationFrame", vi.fn());

    const timeline = document.createElement("ol");
    const node = document.createElement("li");
    node.className = "agent-timeline__node";
    timeline.append(node);
    timeline.getBoundingClientRect = vi.fn(() => ({
      x: 0,
      y: 0,
      top: 0,
      right: 0,
      bottom: 100,
      left: 0,
      width: 10,
      height: 100,
      toJSON: () => ({}),
    }));
    node.getBoundingClientRect = vi.fn(() => ({
      x: 0,
      y: 12,
      top: 12,
      right: 10,
      bottom: 32,
      left: 0,
      width: 10,
      height: 20,
      toJSON: () => ({}),
    }));

    let railStyle: ReturnType<typeof useTimelineRailMask>["railLineStyle"] | null = null;
    const Harness = defineComponent({
      setup() {
        const rail = useTimelineRailMask([]);
        rail.timelineRef.value = timeline;
        railStyle = rail.railLineStyle;
        return () => null;
      },
    });

    const view = render(Harness);
    await nextTick();
    expect(rafCallback).not.toBeNull();

    rafCallback?.(0);
    view.unmount();
    await nextTick();

    expect(railStyle?.value.maskImage).toBeUndefined();
  });
});
