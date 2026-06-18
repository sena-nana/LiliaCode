import { render, waitFor } from "@testing-library/vue";
import { defineComponent, h, ref } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDeferredVisibility } from "../src/components/chat/markdown/useDeferredVisibility";

type MockIntersectionEntry = {
  isIntersecting: boolean;
  intersectionRatio: number;
  target: Element;
};

class MockIntersectionObserver {
  static instances: MockIntersectionObserver[] = [];

  callback: IntersectionObserverCallback;
  observe = vi.fn();
  disconnect = vi.fn();

  constructor(callback: IntersectionObserverCallback) {
    this.callback = callback;
    MockIntersectionObserver.instances.push(this);
  }

  emit(entries: MockIntersectionEntry[]) {
    this.callback(entries as unknown as IntersectionObserverEntry[], this as unknown as IntersectionObserver);
  }
}

const VisibilityHarness = defineComponent({
  setup() {
    const host = ref<HTMLElement | null>(null);
    const { activated } = useDeferredVisibility({
      target: () => host.value,
      perfName: "test.visibility",
      immediate: false,
    });
    return () => h("div", {
      ref: host,
      "data-testid": "host",
      "data-state": activated.value ? "ready" : "idle",
    });
  },
});

describe("useDeferredVisibility", () => {
  beforeEach(() => {
    MockIntersectionObserver.instances.length = 0;
    vi.stubGlobal("IntersectionObserver", MockIntersectionObserver as unknown as typeof IntersectionObserver);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("仅在进入视口后激活", async () => {
    const view = render(VisibilityHarness);
    const host = view.getByTestId("host");

    expect(host).toHaveAttribute("data-state", "idle");
    expect(MockIntersectionObserver.instances).toHaveLength(1);
    expect(MockIntersectionObserver.instances[0]?.observe).toHaveBeenCalledWith(host);

    MockIntersectionObserver.instances[0]?.emit([{
      isIntersecting: false,
      intersectionRatio: 0,
      target: host,
    }]);
    expect(host).toHaveAttribute("data-state", "idle");

    MockIntersectionObserver.instances[0]?.emit([{
      isIntersecting: true,
      intersectionRatio: 1,
      target: host,
    }]);

    await waitFor(() => {
      expect(host).toHaveAttribute("data-state", "ready");
    });
    expect(MockIntersectionObserver.instances[0]?.disconnect).toHaveBeenCalledTimes(1);
  });

  it("缺少 IntersectionObserver 时回退为立即激活", async () => {
    vi.unstubAllGlobals();
    // @ts-expect-error test override
    delete globalThis.IntersectionObserver;

    const view = render(VisibilityHarness);

    await waitFor(() => {
      expect(view.getByTestId("host")).toHaveAttribute("data-state", "ready");
    });
  });
});
