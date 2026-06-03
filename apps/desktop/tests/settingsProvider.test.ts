import { fireEvent, render, waitFor, within } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import Settings from "../src/pages/Settings.vue";
import {
  failNextPopupSettingsSave,
  mockInvoke,
  setMockActiveBackend,
  setMockCodexAppServerStatus,
} from "./tauriMock";

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
      expect(view.getByText(/OpenAI Responses API/)).toBeInTheDocument();
    });
  });

  it("弹出窗口快捷键默认关闭，可录入并保存全局快捷键", async () => {
    const view = render(Settings);
    const input = view.getByLabelText("弹出窗口快捷键") as HTMLInputElement;
    const card = input.closest(".card") as HTMLElement;

    expect(input.value).toBe("未设置");

    await fireEvent.keyDown(input, {
      key: "l",
      ctrlKey: true,
      shiftKey: true,
    });

    expect(input.value).toBe("Ctrl+Shift+L");

    await fireEvent.click(within(card).getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("popup_set_window_settings", {
        settings: { shortcut: "Ctrl+Shift+L" },
      }, undefined);
    });
  });

  it("弹出窗口快捷键注册失败时显示错误并保留表单值", async () => {
    failNextPopupSettingsSave("快捷键已被占用");
    const view = render(Settings);
    const input = view.getByLabelText("弹出窗口快捷键") as HTMLInputElement;
    const card = input.closest(".card") as HTMLElement;

    await fireEvent.keyDown(input, {
      key: "p",
      ctrlKey: true,
      altKey: true,
    });
    await fireEvent.click(within(card).getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(view.getByText(/快捷键已被占用/)).toBeInTheDocument();
    });
    expect(input.value).toBe("Ctrl+Alt+P");
  });
});
