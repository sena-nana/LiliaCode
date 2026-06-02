import { fireEvent, render, waitFor } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import Settings from "../src/pages/Settings.vue";
import { mockInvoke, setMockActiveBackend, setMockCodexAppServerStatus } from "./tauriMock";

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

  it("Codex app-server 环境不满足时在设置页连接 banner 显示原因", async () => {
    setMockActiveBackend("codex");
    setMockCodexAppServerStatus({
      supportsRequiredProtocol: false,
      issues: ["当前 codex CLI 版本过低，需要 0.128.0 或更新版本。"],
    });

    const view = render(Settings);

    await waitFor(() => {
      expect(view.getByText("Codex 运行环境不满足")).toBeInTheDocument();
      expect(view.getByText(/当前 codex CLI 版本过低/)).toBeInTheDocument();
    });
  });
});
