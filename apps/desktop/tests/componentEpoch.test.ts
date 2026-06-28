import { render } from "@testing-library/vue";
import { defineComponent, h } from "vue";
import { describe, expect, it } from "vitest";
import { withComponentEpoch, type ComponentEpoch } from "../src/composables/useComponentEpoch";

describe("withComponentEpoch", () => {
  it("tracks alive state and invalidates epochs on demand", () => {
    let epoch: ComponentEpoch | null = null;
    const Host = defineComponent({
      setup() {
        epoch = withComponentEpoch();
        return () => h("div");
      },
    });

    render(Host);

    const first = epoch?.nextEpoch();
    expect(first).toBe(1);
    expect(epoch?.assertAlive(first)).toBe(true);

    epoch?.invalidate();

    expect(epoch?.assertAlive(first)).toBe(false);
    expect(epoch?.assertAlive()).toBe(true);
  });

  it("marks pending epochs stale after unmount", () => {
    let epoch: ComponentEpoch | null = null;
    const Host = defineComponent({
      setup() {
        epoch = withComponentEpoch();
        return () => h("div");
      },
    });

    const view = render(Host);
    const pending = epoch?.nextEpoch();

    view.unmount();

    expect(epoch?.assertAlive()).toBe(false);
    expect(epoch?.assertAlive(pending)).toBe(false);
  });
});

