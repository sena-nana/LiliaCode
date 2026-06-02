import { fireEvent, render, waitFor } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import Settings from "../src/pages/Settings.vue";
import { mockInvoke } from "./tauriMock";

describe("Settings provider switch", () => {
  it("点击 Codex 会写入全局 active provider", async () => {
    const view = render(Settings);

    await fireEvent.click(view.getByRole("radio", { name: "Codex" }));

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd, args]) =>
          cmd === "provider_set_active_backend" &&
          typeof args === "object" &&
          args !== null &&
          "backend" in args &&
          args.backend === "codex"
        ),
      ).toBe(true);
    });
    expect(view.getByRole("radio", { name: "Codex" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });
});
