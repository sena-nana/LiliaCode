import { fireEvent, render, waitFor, within } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { describe, expect, it } from "vitest";
import Settings from "../src/pages/Settings.vue";
import { createLiliaRouter } from "../src/router";
import {
  failNextPopupSettingsSave,
  mockInvoke,
  setMockActiveBackend,
  setMockCodexAppServerStatus,
} from "./tauriMock";

async function renderSettings(initialRoute = "/settings") {
  const router = createLiliaRouter(createMemoryHistory());
  await router.push(initialRoute);
  await router.isReady();

  return render(Settings, {
    global: {
      plugins: [router],
    },
  });
}

describe("Settings provider switch", () => {
  it("非法 tab 默认显示外观分类", async () => {
    const invalid = await renderSettings("/settings?tab=unknown");
    expect(invalid.getByText("外观")).toBeInTheDocument();
    expect(invalid.getByLabelText("弹出窗口快捷键")).toBeInTheDocument();
    expect(invalid.queryByRole("radio", { name: "Codex" })).not.toBeInTheDocument();
  });

  it("点击 Codex 会写入全局 active provider", async () => {
    const view = await renderSettings("/settings?tab=providers");

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
    await waitFor(() => {
      expect(view.getByRole("radio", { name: "Codex" })).toHaveAttribute(
        "aria-checked",
        "true",
      );
    });
  });

  it("Codex CLI 版本过低时设置页连接 banner 不显示 provider 兼容提示", async () => {
    setMockActiveBackend("codex");
    setMockCodexAppServerStatus({
      supportsRequiredProtocol: false,
      failureKind: "experimentalApiUnsupported",
      issues: ["当前 codex CLI 版本过低，需要 0.128.0 或更新版本。"],
    });

    const view = await renderSettings("/settings?tab=providers");

    await waitFor(() => {
      expect(view.getByText("Codex 运行环境不满足")).toBeInTheDocument();
      expect(view.getByText(/当前 codex CLI 版本过低/)).toBeInTheDocument();
      expect(view.queryByText(/未找到 codex CLI/)).not.toBeInTheDocument();
      expect(view.queryByText(/OpenAI Responses API/)).not.toBeInTheDocument();
    });
  });

  it("Codex provider 不兼容时设置页连接 banner 显示 Responses API 提示", async () => {
    setMockActiveBackend("codex");
    setMockCodexAppServerStatus({
      supportsRequiredProtocol: false,
      failureKind: "providerIncompatible",
      issues: ["当前上游 provider 不兼容 Codex。"],
    });

    const view = await renderSettings("/settings?tab=providers");

    await waitFor(() => {
      expect(view.getByText("Codex 运行环境不满足")).toBeInTheDocument();
      expect(view.getByText(/OpenAI Responses API/)).toBeInTheDocument();
      expect(view.getByText(/模型白名单/)).toBeInTheDocument();
    });
  });

  it("弹出窗口快捷键默认关闭，可录入并保存全局快捷键", async () => {
    const view = await renderSettings();
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
    const view = await renderSettings();
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

  it("Codex profile settings 保存受控 permissions 与 workspace roots", async () => {
    const view = await renderSettings("/settings?tab=agent");

    await fireEvent.click(view.getByRole("radio", { name: "High" }));
    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd, args]) =>
          cmd === "agent_interaction_set_settings" &&
          typeof args === "object" &&
          args !== null &&
          "settings" in args &&
          JSON.stringify(args.settings).includes("\"reasoningEffort\":\"high\"")
        ),
      ).toBe(true);
    });
    await fireEvent.click(view.getByRole("radio", { name: "工作区" }));
    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd, args]) =>
          cmd === "agent_interaction_set_settings" &&
          typeof args === "object" &&
          args !== null &&
          "settings" in args &&
          JSON.stringify(args.settings).includes("\"permissions\":{\"profile\":\"workspaceWrite\"}")
        ),
      ).toBe(true);
    });

    const roots = view.container.querySelector(".settings-input--textarea") as HTMLTextAreaElement;
    await fireEvent.update(roots, "C:/repo\nC:/repo\nD:/shared\n");
    await fireEvent.blur(roots);

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd, args]) =>
          cmd === "agent_interaction_set_settings" &&
          typeof args === "object" &&
          args !== null &&
          "settings" in args &&
          JSON.stringify(args.settings).includes("\"runtimeWorkspaceRoots\":[\"C:/repo\",\"D:/shared\"]")
        ),
      ).toBe(true);
    });
  });

  it("新对话建议生成来源可切换到当前 Provider", async () => {
    const view = await renderSettings("/settings?tab=providers");

    await fireEvent.click(view.getByRole("radio", { name: "当前 Provider" }));

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd, args]) =>
          cmd === "conversation_suggestions_set_settings" &&
          typeof args === "object" &&
          args !== null &&
          "settings" in args &&
          JSON.stringify(args.settings).includes("\"source\":\"provider\"")
        ),
      ).toBe(true);
    });
  });
});
