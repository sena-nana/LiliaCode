import { fireEvent, render, waitFor, within } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  ASSISTANT_AI_GET_CONFIG_COMMAND,
  ASSISTANT_AI_SET_CONFIG_COMMAND,
  PROVIDER_CODEX_ACCOUNT_START_LOGIN_COMMAND,
  GITHUB_POLL_DEVICE_FLOW_COMMAND,
  GITHUB_START_DEVICE_FLOW_COMMAND,
  POPUP_SET_WINDOW_SETTINGS_COMMAND,
  PROJECT_GET_SETTINGS_COMMAND,
  PROJECT_SET_SETTINGS_COMMAND,
  QUOTA_USAGE_CONSUME_CODEX_RATE_LIMIT_RESET_CREDIT_COMMAND,
  QUOTA_USAGE_GET_CODEX_ACCOUNT_STATUS_COMMAND,
  QUOTA_USAGE_GET_STATS_COMMAND,
  REMOTE_CONTROL_STATUS_COMMAND,
  SYSTEM_OPEN_URL_COMMAND,
} from "@lilia/contracts";
import Settings from "../src/pages/Settings.vue";
import { createLiliaRouter } from "../src/router";
import { TAURI_PLUGIN_DIALOG_OPEN_COMMAND } from "../src/tauri/pluginCommands";
import {
  failNextPopupSettingsSave,
  mockInvoke,
  setMockGitHubPollSequence,
  setMockActiveBackend,
  setMockCodexAppServerStatus,
  setMockCodexAccountQuotaStatus,
  setMockProviderConfig,
  setMockQuotaUsageStats,
  setMockRouterMode,
} from "./tauriMock";

async function renderSettings(initialRoute = "/settings") {
  const router = createLiliaRouter(createMemoryHistory());
  await router.push(initialRoute);
  await router.isReady();

  const view = render(Settings, {
    global: {
      plugins: [router],
    },
  });
  if (typeof vi.dynamicImportSettled === "function") {
    await vi.dynamicImportSettled();
  }
  await Promise.resolve();
  await Promise.resolve();
  return { ...view, router };
}

function lastInvokeInput(command: string): Record<string, unknown> | undefined {
  const call = [...mockInvoke.mock.calls].reverse().find(([cmd]) => cmd === command);
  const input = call?.[1];
  return input && typeof input === "object" ? input as Record<string, unknown> : undefined;
}

function emptyQuotaStats() {
  const dayMs = 86_400_000;
  const rangeEnd = Math.floor(Date.now() / dayMs) * dayMs + dayMs;
  const rangeStart = rangeEnd - 7 * dayMs;
  return {
    days: 7,
    backend: "all",
    rangeStart,
    rangeEnd,
    totals: {
      inputTokens: 0,
      outputTokens: 0,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
      totalTokens: 0,
    },
    cost: {
      knownCostUsd: null,
      costRecordCount: 0,
      totalRecordCount: 0,
    },
    daily: Array.from({ length: 7 }, (_, index) => ({
      dayStart: rangeStart + index * dayMs,
      inputTokens: 0,
      outputTokens: 0,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
      totalTokens: 0,
      knownCostUsd: null,
      costRecordCount: 0,
      recordCount: 0,
    })),
    backends: [],
    recent: [],
    projects: [],
    conversations: [],
    tools: [],
  };
}

describe("Settings provider switch", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("卸载时取消设置 tab paint 打点调度", async () => {
    const cancelAnimationFrame = vi.fn();
    vi.stubGlobal("requestAnimationFrame", vi.fn(() => 53));
    vi.stubGlobal("cancelAnimationFrame", cancelAnimationFrame);

    const view = await renderSettings("/settings?tab=appearance");
    view.unmount();

    expect(cancelAnimationFrame).toHaveBeenCalledWith(53);
  });

  it("非法 tab 默认显示外观分类", async () => {
    const invalid = await renderSettings("/settings?tab=unknown");
    expect(invalid.getByRole("heading", { level: 2, name: "外观" })).toBeInTheDocument();
    expect(invalid.queryByRole("heading", { level: 1 })).not.toBeInTheDocument();
    expect(invalid.queryByLabelText("弹出窗口快捷键")).not.toBeInTheDocument();
    expect(invalid.queryByRole("radio", { name: "Codex" })).not.toBeInTheDocument();
  });

  it("旧 Codex 会话 tab 回落到外观分类", async () => {
    const view = await renderSettings("/settings?tab=codex-sessions");

    expect(view.getByRole("heading", { level: 2, name: "外观" })).toBeInTheDocument();
    expect(view.queryByRole("heading", { level: 1 })).not.toBeInTheDocument();
    expect(view.queryByText("Codex 会话管理")).not.toBeInTheDocument();
  });

  it("插件技能和导入对话保持原页面主内容布局", async () => {
    const plugins = await renderSettings("/settings?tab=plugins");
    expect(plugins.container.querySelector(".plugins-page")).toBeInTheDocument();
    expect(plugins.queryByRole("heading", { level: 1, name: "插件 / 技能" }))
      .not.toBeInTheDocument();

    const imports = await renderSettings("/settings?tab=import");
    expect(imports.container.querySelector(".conversation-import-page")).toBeInTheDocument();
    expect(imports.queryByRole("heading", { level: 1, name: "导入对话" }))
      .not.toBeInTheDocument();
  });

  it("设置页只在切到对应 tab 时加载重量 section", async () => {
    mockInvoke.mockClear();
    const view = await renderSettings("/settings?tab=appearance");

    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === QUOTA_USAGE_GET_STATS_COMMAND)).toBe(false);

    await view.router.push("/settings?tab=quota");
    await view.router.isReady();

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === QUOTA_USAGE_GET_STATS_COMMAND)).toBe(true);
    });
  });

  it("连接页初始化不会挂载 Android 远控或刷新远控状态", async () => {
    mockInvoke.mockClear();
    const view = await renderSettings("/settings?tab=providers");

    await waitFor(() => {
      expect(view.getByRole("heading", { level: 2, name: "连接" })).toBeInTheDocument();
    });
    expect(view.queryByRole("heading", { level: 2, name: "Android 远控" })).not.toBeInTheDocument();
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === REMOTE_CONTROL_STATUS_COMMAND)).toBe(false);
  });

  it("外观页可以切换侧边栏样式并写入本地偏好", async () => {
    const view = await renderSettings("/settings?tab=appearance");

    expect(view.getByRole("radio", { name: "按项目分组" })).toHaveAttribute(
      "aria-checked",
      "true",
    );

    await fireEvent.click(view.getByRole("radio", { name: "统一列表" }));

    expect(view.getByRole("radio", { name: "统一列表" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    expect(localStorage.getItem("lilia.sidebarDisplayMode")).toBe("unified");
  });

  it("外观页可以切换超椭圆圆角并调整全局半径", async () => {
    const view = await renderSettings("/settings?tab=appearance");

    expect(view.getByRole("radio", { name: "平滑" })).toHaveAttribute("aria-checked", "true");
    expect(document.documentElement.dataset.corners).toBe("smooth");
    expect(document.documentElement.style.getPropertyValue("--app-corner-radius")).toBe("8px");

    await fireEvent.click(view.getByRole("radio", { name: "普通" }));
    expect(view.getByRole("radio", { name: "普通" })).toHaveAttribute("aria-checked", "true");
    expect(document.documentElement.dataset.corners).toBe("round");
    expect(localStorage.getItem("lilia.corners")).toBe("round");

    await fireEvent.input(view.getByRole("slider", { name: "圆角半径" }), {
      target: { value: "14" },
    });
    expect(document.documentElement.style.getPropertyValue("--app-corner-radius")).toBe("14px");
    expect(localStorage.getItem("lilia.cornerRadius")).toBe("14");
  });

  it("额度页显示近 7 天 Token 和成本统计", async () => {
    const view = await renderSettings("/settings?tab=quota");

    expect(view.queryByRole("heading", { level: 1 })).not.toBeInTheDocument();
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(QUOTA_USAGE_GET_STATS_COMMAND, {
        input: { days: 7, backend: "all" },
      }, undefined);
    });
    expect(await view.findByText("总 Token")).toBeInTheDocument();
    expect(
      view.getByRole("img", { name: "Token 用量趋势（全部 · 近 7 天）" }),
    ).toBeInTheDocument();
    expect(view.getByRole("img", { name: "后端 Token 分布" })).toBeInTheDocument();
    expect(view.getByText("项目消耗")).toBeInTheDocument();
    expect(view.getByRole("img", { name: "项目消耗图表" })).toBeInTheDocument();
    expect(view.getByText("对话消耗")).toBeInTheDocument();
    expect(view.getByRole("img", { name: "对话消耗图表" })).toBeInTheDocument();
    expect(view.getByText("工具活跃度")).toBeInTheDocument();
    expect(view.getByRole("img", { name: "工具活跃度图表" })).toBeInTheDocument();
    expect(view.getByText("按调用次数统计")).toBeInTheDocument();

    await fireEvent.click(view.getByRole("radio", { name: "Codex" }));

    await waitFor(() => {
      expect(lastInvokeInput(QUOTA_USAGE_GET_STATS_COMMAND)).toMatchObject({
        input: { days: 7, backend: "codex" },
      });
    });
    expect(
      view.getByRole("img", { name: "Token 用量趋势（Codex · 近 7 天）" }),
    ).toBeInTheDocument();

    await fireEvent.click(view.getByRole("radio", { name: "30 天" }));

    await waitFor(() => {
      expect(lastInvokeInput(QUOTA_USAGE_GET_STATS_COMMAND)).toMatchObject({
        input: { days: 30, backend: "codex" },
      });
    });
    expect(
      view.getByRole("img", { name: "Token 用量趋势（Codex · 近 30 天）" }),
    ).toBeInTheDocument();
  });

  it("额度页无新增记录时显示空态", async () => {
    setMockQuotaUsageStats(emptyQuotaStats());

    const view = await renderSettings("/settings?tab=quota");

    expect(await view.findAllByText("暂无新增额度数据")).toHaveLength(2);
    expect(view.getByText("无新增记录")).toBeInTheDocument();
    expect(view.queryByRole("img", { name: /Token 用量趋势/ })).not.toBeInTheDocument();
  });

  it("额度页在 Codex 官方账号模式显示官方额度", async () => {
    const view = await renderSettings("/settings?tab=quota");

    expect(await view.findByText("Codex 官方额度")).toBeInTheDocument();
    expect(view.queryByText("5 小时限额")).not.toBeInTheDocument();
    expect(view.queryByText("周限额")).not.toBeInTheDocument();
    expect(view.getByText("25%")).toBeInTheDocument();
    expect(view.getByText("40%")).toBeInTheDocument();
    expect(view.getByText("剩余 75%")).toBeInTheDocument();
    expect(view.getByText("剩余 60%")).toBeInTheDocument();
    expect(view.getByText("Spark额度")).toBeInTheDocument();
    expect(view.getByText("15%")).toBeInTheDocument();
    expect(view.getByText("70%")).toBeInTheDocument();
    expect(view.getByText("剩余 85%")).toBeInTheDocument();
    expect(view.getByText("剩余 30%")).toBeInTheDocument();
    expect(view.getAllByText(/^重置 /)).toHaveLength(4);
    expect(view.getByText("重置次数 可用 2 次")).toBeInTheDocument();
    expect(view.getByRole("button", { name: "使用重置次数" })).toBeEnabled();
    expect(view.getByText("Workspace credit")).toBeInTheDocument();
    expect(view.getByText("剩余 3")).toBeInTheDocument();
    expect(view.getByText("Spark workspace credit")).toBeInTheDocument();
    expect(view.getByText("不限")).toBeInTheDocument();
    expect(view.getByText("官方账号用量")).toBeInTheDocument();
    expect(view.getByText("累计 Token")).toBeInTheDocument();
    expect(view.getByText("连续活跃")).toBeInTheDocument();
  });

  it("额度页在 Codex API 模式隐藏官方额度", async () => {
    setMockRouterMode("codex", "api");

    const view = await renderSettings("/settings?tab=quota");

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        QUOTA_USAGE_GET_CODEX_ACCOUNT_STATUS_COMMAND,
        {},
        undefined,
      );
    });
    expect(view.queryByText("Codex 官方额度")).not.toBeInTheDocument();
  });

  it("额度 mock 在 Codex API 未配置时返回真实连接模式", async () => {
    setMockRouterMode("codex", "api");
    setMockProviderConfig("codex", { baseUrl: null, hasApiKey: false });

    const status = await mockInvoke(QUOTA_USAGE_GET_CODEX_ACCOUNT_STATUS_COMMAND, {}, undefined);

    expect(status).toMatchObject({
      available: false,
      connectionMode: "unconfigured",
    });
  });

  it("额度页刷新按钮同时刷新本地统计和官方额度", async () => {
    const view = await renderSettings("/settings?tab=quota");
    await view.findByText("Codex 官方额度");
    mockInvoke.mockClear();

    await fireEvent.click(view.getByRole("button", { name: "刷新" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(QUOTA_USAGE_GET_STATS_COMMAND, {
        input: { days: 7, backend: "all" },
      }, undefined);
      expect(mockInvoke).toHaveBeenCalledWith(
        QUOTA_USAGE_GET_CODEX_ACCOUNT_STATUS_COMMAND,
        {},
        undefined,
      );
    });
  });

  it("额度页可以使用 Codex 官方重置次数并刷新官方额度", async () => {
    const view = await renderSettings("/settings?tab=quota");
    await view.findByText("Codex 官方额度");
    mockInvoke.mockClear();

    await fireEvent.click(view.getByRole("button", { name: "使用重置次数" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        QUOTA_USAGE_CONSUME_CODEX_RATE_LIMIT_RESET_CREDIT_COMMAND,
        { input: { idempotencyKey: expect.any(String) } },
        undefined,
      );
    });
    expect(await view.findByText("已使用 1 次重置次数，官方额度已刷新")).toBeInTheDocument();
    expect(view.getByText("重置次数 可用 1 次")).toBeInTheDocument();
  });

  it("额度页官方额度失败不影响本地统计", async () => {
    setMockCodexAccountQuotaStatus({
      available: false,
      connectionMode: "codex-account",
      limitId: "codex",
      limitName: null,
      planType: "pro",
      rateLimitReachedType: null,
      fiveHour: null,
      weekly: null,
      sparkFiveHour: null,
      sparkWeekly: null,
      credits: null,
      sparkCredits: null,
      rateLimitResetCredits: null,
      accountUsage: null,
      usageError: null,
      fetchedAt: Date.now(),
      error: "Codex 未登录",
    });

    const view = await renderSettings("/settings?tab=quota");

    expect(await view.findByText("总 Token")).toBeInTheDocument();
    expect(await view.findByText("Codex 官方额度")).toBeInTheDocument();
    expect(view.getByText("暂无官方额度数据")).toBeInTheDocument();
    expect(view.getByText("Codex 未登录")).toBeInTheDocument();
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

  it("连接页默认使用 API/官方账号，不再展示 CC-Switch 专用配置", async () => {
    setMockActiveBackend("codex");

    const view = await renderSettings("/settings?tab=providers");

    await waitFor(() => {
      expect(view.getByRole("radio", { name: "官方账号" })).toHaveAttribute(
        "aria-checked",
        "true",
      );
    });
    expect(view.queryByText("CC-Switch")).not.toBeInTheDocument();
    expect(
      mockInvoke.mock.calls.some(([cmd, args]) =>
        cmd === "router_set_mode" &&
        typeof args === "object" &&
        args !== null &&
        "mode" in args &&
        args.mode === "cc-switch"
      ),
    ).toBe(false);
  });

  it("API 模式可以保存 Base URL 和 API key，空 key 保存保留已有密钥", async () => {
    setMockRouterMode("claude", "api");
    setMockProviderConfig("claude", { baseUrl: "https://api.anthropic.com", hasApiKey: true });

    const view = await renderSettings("/settings?tab=providers");

    const baseUrlInput = await view.findByPlaceholderText("https://api.anthropic.com") as HTMLInputElement;
    const apiKeyInput = await view.findByPlaceholderText("已保存，留空保留现有值") as HTMLInputElement;

    await fireEvent.update(baseUrlInput, "https://anthropic.example/v1");
    await fireEvent.update(apiKeyInput, "");
    await fireEvent.click(view.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(lastInvokeInput("provider_set_config")).toMatchObject({
        config: {
          baseUrl: "https://anthropic.example/v1",
          apiKey: null,
        },
      });
      expect(lastInvokeInput("provider_set_config")?.config).not.toMatchObject({
        clearApiKey: true,
      });
    });
    await waitFor(() => {
      expect(view.getByRole("button", { name: "保存" })).toBeEnabled();
    });
    expect(view.getByText("密钥已保存")).toBeInTheDocument();

    await fireEvent.update(apiKeyInput, "sk-new");
    await fireEvent.click(view.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(lastInvokeInput("provider_set_config")).toMatchObject({
        config: { apiKey: "sk-new" },
      });
    });
  });

  it("连接页卸载后保存完成不会继续刷新 provider 配置", async () => {
    setMockRouterMode("claude", "api");
    setMockProviderConfig("claude", { baseUrl: "https://api.anthropic.com", hasApiKey: true });
    const originalInvoke = vi.mocked(mockInvoke).getMockImplementation();
    const view = await renderSettings("/settings?tab=providers");
    const baseUrlInput = await view.findByPlaceholderText("https://api.anthropic.com") as HTMLInputElement;
    let resolveSave: () => void;
    vi.mocked(mockInvoke).mockImplementation((cmd: string, args: Record<string, unknown> = {}) => {
      if (cmd === "provider_set_config") {
        return new Promise((resolve) => {
          resolveSave = () => resolve(undefined);
        });
      }
      return originalInvoke?.(cmd, args) ?? Promise.resolve(null);
    });

    try {
      mockInvoke.mockClear();
      await fireEvent.update(baseUrlInput, "https://anthropic-late.example/v1");
      await fireEvent.click(view.getByRole("button", { name: "保存" }));
      await waitFor(() => {
        expect(typeof resolveSave).toBe("function");
      });

      view.unmount();
      resolveSave!();
      await Promise.resolve();
      await Promise.resolve();

      const commandsAfterSave = mockInvoke.mock.calls.map(([cmd]) => cmd);
      expect(commandsAfterSave).toContain("provider_set_config");
      expect(commandsAfterSave.filter((cmd) => cmd === "provider_get_config")).toHaveLength(0);
    } finally {
      vi.mocked(mockInvoke).mockImplementation(originalInvoke);
    }
  });

  it("Codex 切换到 API 模式后可以显式清除已保存的 API key", async () => {
    setMockRouterMode("codex", "api");
    setMockActiveBackend("codex");
    setMockProviderConfig("codex", { hasApiKey: true });

    const view = await renderSettings("/settings?tab=providers");
    await waitFor(() => {
      expect(view.getByText("密钥已保存")).toBeInTheDocument();
      expect(view.getByRole("button", { name: "清除" })).toBeEnabled();
    });
    await fireEvent.click(view.getByRole("button", { name: "清除" }));

    await waitFor(() => {
      expect(lastInvokeInput("provider_set_config")).toMatchObject({
        config: { clearApiKey: true },
      });
      expect(view.getByText("未保存密钥")).toBeInTheDocument();
    });
  });

  it("Codex app-server 缺失时显示内置安装建议", async () => {
    setMockActiveBackend("codex");
    setMockCodexAppServerStatus({
      available: false,
      supportsRequiredProtocol: false,
      failureKind: "missingCli",
      issues: ["未找到 Lilia 内置 Codex app-server。"],
    });

    const view = await renderSettings("/settings?tab=providers");

    await waitFor(() => {
      expect(view.getByText("Codex app-server 缺失")).toBeInTheDocument();
      expect(view.getAllByText(/Lilia 内置 Codex app-server/).length).toBeGreaterThan(0);
      expect(view.queryByText(/npm i -g @openai\/codex/)).not.toBeInTheDocument();
      expect(view.queryByText(/刷新 PATH/)).not.toBeInTheDocument();
    });
  });

  it("Codex CLI 版本过低时设置页连接 banner 不显示 provider 兼容提示", async () => {
    setMockActiveBackend("codex");
    setMockCodexAppServerStatus({
      supportsRequiredProtocol: false,
      failureKind: "experimentalApiUnsupported",
      issues: ["当前 codex CLI 版本过低，需要 0.136.0 或更新版本。"],
    });

    const view = await renderSettings("/settings?tab=providers");

    await waitFor(() => {
      expect(view.getByText("Codex app-server 不可用")).toBeInTheDocument();
      expect(view.getAllByText(/当前 codex CLI 版本过低/).length).toBeGreaterThan(0);
      expect(view.queryByText(/未找到 codex CLI/)).not.toBeInTheDocument();
      expect(view.getAllByText(/更新 Lilia 内置 Codex app-server/).length).toBeGreaterThan(0);
      expect(view.queryByText(/模型白名单/)).not.toBeInTheDocument();
    });
  });

  it("Codex 官方账号模式可以从设置页更新 app-server", async () => {
    setMockActiveBackend("codex");
    setMockRouterMode("codex", "codex-account");
    setMockCodexAppServerStatus({
      version: "codex-cli 0.136.0",
      installPath: null,
      managed: false,
      latestVersion: "0.141.0",
      updateAvailable: true,
      updateState: "ready",
      preparedVersion: "0.141.0",
      releaseNotes: ["App-server update"],
    });

    const view = await renderSettings("/settings?tab=providers");

    await waitFor(() => {
      expect(view.getByText("当前版本：codex-cli 0.136.0 / latest 0.141.0")).toBeInTheDocument();
      expect(view.queryByText("运行时状态")).not.toBeInTheDocument();
      expect(view.queryByText(/路径：/)).not.toBeInTheDocument();
      expect(view.queryByText("将安装到 Lilia 管理目录")).not.toBeInTheDocument();
      expect(view.container.querySelector("[data-agent-id='settings.provider.probe']")).toBeNull();
      expect(view.getByRole("button", { name: /切换到 0.141.0/ })).toBeEnabled();
    });

    await fireEvent.click(view.getByRole("button", { name: /切换到 0.141.0/ }));

    await waitFor(() => {
      expect(lastInvokeInput("provider_codex_app_server_install_update")).toEqual({});
      expect(view.queryByRole("button", { name: /切换到 0.141.0/ })).not.toBeInTheDocument();
    });
  });

  it("Codex app-server 后台下载中时设置页禁用切换按钮", async () => {
    setMockActiveBackend("codex");
    setMockRouterMode("codex", "codex-account");
    setMockCodexAppServerStatus({
      version: "codex-cli 0.136.0",
      latestVersion: "0.141.0",
      updateAvailable: true,
      updateState: "downloading",
      preparedVersion: "0.141.0",
    });

    const view = await renderSettings("/settings?tab=providers");

    const button = await view.findByRole("button", { name: /下载中/ });
    expect(button).toBeDisabled();
  });

  it("Codex 官方账号未登录时在运行时状态显示登录按钮", async () => {
    setMockActiveBackend("codex");
    setMockRouterMode("codex", "codex-account");
    setMockCodexAccountQuotaStatus({
      available: false,
      connectionMode: "codex-account",
      limitId: null,
      limitName: null,
      planType: null,
      rateLimitReachedType: null,
      fiveHour: null,
      weekly: null,
      sparkFiveHour: null,
      sparkWeekly: null,
      credits: null,
      sparkCredits: null,
      rateLimitResetCredits: null,
      accountUsage: null,
      usageError: null,
      fetchedAt: Date.now(),
      error: "Codex 未登录",
    });

    const view = await renderSettings("/settings?tab=providers");

    await waitFor(() => {
      expect(view.queryByText("运行时状态")).not.toBeInTheDocument();
      expect(view.getByText("登录状态：未登录")).toBeInTheDocument();
      expect(view.queryByText(/路径：/)).not.toBeInTheDocument();
      expect(view.queryByText("将安装到 Lilia 管理目录")).not.toBeInTheDocument();
      expect(view.container.querySelector("[data-agent-id='settings.provider.probe']")).toBeNull();
      expect(view.getByRole("button", { name: "登录" })).toBeEnabled();
    });

    await fireEvent.click(view.getByRole("button", { name: "登录" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        PROVIDER_CODEX_ACCOUNT_START_LOGIN_COMMAND,
        {},
        undefined,
      );
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
      expect(view.getByText("Codex app-server 不可用")).toBeInTheDocument();
      expect(view.getAllByText(/OpenAI Responses API/).length).toBeGreaterThan(0);
      expect(view.getAllByText(/模型白名单/).length).toBeGreaterThan(0);
    });
  });

  it("自定义 API 来源未设置密钥时不显示未配置", async () => {
    setMockRouterMode("claude", "api");
    setMockProviderConfig("claude", { baseUrl: "http://127.0.0.1:15721", hasApiKey: false });

    const view = await renderSettings("/settings?tab=providers");

    await waitFor(() => {
      expect(view.getByText("Claude 自定义 API 来源")).toBeInTheDocument();
      expect(view.getAllByText(/未设置密钥/).length).toBeGreaterThan(0);
      expect(view.queryByText(/未配置/)).not.toBeInTheDocument();
    });
  });

  it("弹出窗口快捷键默认关闭，可录入并保存全局快捷键", async () => {
    const view = await renderSettings("/settings?tab=window");
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
      expect(mockInvoke).toHaveBeenCalledWith(POPUP_SET_WINDOW_SETTINGS_COMMAND, {
        settings: { shortcut: "Ctrl+Shift+L" },
      }, undefined);
    });
  });

  it("弹出窗口快捷键注册失败时显示错误并保留表单值", async () => {
    failNextPopupSettingsSave("快捷键已被占用");
    const view = await renderSettings("/settings?tab=window");
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

  it("Agent 设置页不再显示 Codex profile/model/reasoning/workspace roots/permission 配置", async () => {
    const view = await renderSettings("/settings?tab=agent");

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "agent_interaction_get_settings")).toBe(true);
    });

    expect(view.queryByText("Codex 配置档案")).not.toBeInTheDocument();
    expect(view.queryByText("Codex 模型")).not.toBeInTheDocument();
    expect(view.queryByText("推理强度")).not.toBeInTheDocument();
    expect(view.queryByText("运行时工作区根目录")).not.toBeInTheDocument();
    expect(view.queryByText("Codex 权限")).not.toBeInTheDocument();
    expect(view.queryByText("Codex 高级字段")).not.toBeInTheDocument();
    expect(view.queryByText("扩展历史")).not.toBeInTheDocument();
    expect(view.queryByText("命令执行权限")).not.toBeInTheDocument();
    expect(view.container.querySelector(".ui-textarea")).toBeNull();
  });

  it("Agent 设置页只显示内置协议相关设置", async () => {
    const view = await renderSettings("/settings?tab=agent");

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd]) => cmd === "agent_interaction_get_settings"),
      ).toBe(true);
    });

    expect(view.queryByText("Agent 交互")).toBeInTheDocument();
    expect(view.queryByText("权限行为")).toBeInTheDocument();
    expect(view.queryByText("主 Agent 策略")).toBeInTheDocument();
    expect(view.queryByText("主 Agent 工作流提示预览")).toBeInTheDocument();
    expect(view.getByRole("radio", { name: "保守" })).toHaveAttribute("aria-checked", "true");
    expect(view.getByRole("radio", { name: "激进" })).toBeInTheDocument();
    expect(view.getByRole("radio", { name: "自定义" })).toBeInTheDocument();
    expect(view.getByText(/不替代当前 provider 的原生系统提示/)).toBeInTheDocument();
    expect(view.getByRole("radio", { name: "询问" })).toHaveAttribute("aria-checked", "true");
    expect(view.getByRole("radio", { name: "只读" })).toBeInTheDocument();
    expect(view.getByRole("radio", { name: "完全访问" })).toBeInTheDocument();
    expect(view.getByRole("radio", { name: "自由实现" })).toBeInTheDocument();
    expect(view.getByText("完全访问，并在 8 秒后按建议项处理交互。")).toBeInTheDocument();
  });

  it("Agent 设置页可以保存自由实现权限行为", async () => {
    const view = await renderSettings("/settings?tab=agent");

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd]) => cmd === "agent_interaction_get_settings"),
      ).toBe(true);
    });

    await fireEvent.click(view.getByRole("radio", { name: "自由实现" }));

    await waitFor(() => {
      expect(lastInvokeInput("agent_interaction_set_settings")).toMatchObject({
        settings: expect.objectContaining({
          permissionMode: "free",
        }),
      });
    });
  });

  it("Agent 设置页可以保存主 Agent 激进策略", async () => {
    const view = await renderSettings("/settings?tab=agent");

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd]) => cmd === "agent_interaction_get_settings"),
      ).toBe(true);
    });

    await fireEvent.click(view.getByRole("radio", { name: "激进" }));

    await waitFor(() => {
      expect(lastInvokeInput("agent_interaction_set_settings")).toMatchObject({
        settings: expect.objectContaining({
          mainAgentPromptMode: "aggressive",
        }),
      });
    });
  });

  it("Agent 设置页可以编辑并保存自定义主 Agent 提示词", async () => {
    const view = await renderSettings("/settings?tab=agent");

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd]) => cmd === "agent_interaction_get_settings"),
      ).toBe(true);
    });

    await fireEvent.click(view.getByRole("radio", { name: "自定义" }));

    const textarea = await view.findByLabelText("自定义主 Agent 提示词");
    expect((textarea as HTMLTextAreaElement).value).toContain("主 Agent 策略：保守");
    expect(view.getByText(/不替代当前 provider 的原生系统提示/)).toBeInTheDocument();
    await waitFor(() => {
      expect(lastInvokeInput("agent_interaction_set_settings")).toMatchObject({
        settings: expect.objectContaining({
          mainAgentPromptMode: "custom",
          mainAgentCustomPrompt: expect.stringContaining("主 Agent 策略：保守"),
        }),
      });
    });

    await fireEvent.update(textarea, "按用户的项目约束优先实现。\n必要时重构。");
    await fireEvent.click(view.getByRole("button", { name: "应用自定义提示词" }));

    await waitFor(() => {
      expect(lastInvokeInput("agent_interaction_set_settings")).toMatchObject({
        settings: expect.objectContaining({
          mainAgentPromptMode: "custom",
          mainAgentCustomPrompt: "按用户的项目约束优先实现。\n必要时重构。",
        }),
      });
    });
    expect(view.getByText(/按用户的项目约束优先实现/)).toBeInTheDocument();
  });

  it("Agent 设置页会在首屏后再加载自定义 Agent 目录", async () => {
    vi.useFakeTimers();
    mockInvoke.mockClear();

    const view = await renderSettings("/settings?tab=agent");

    expect(
      mockInvoke.mock.calls.some(([cmd]) => cmd === "agent_interaction_list_subagents"),
    ).toBe(false);
    expect(view.getByText("首屏加载后补齐自定义 Agent 目录…")).toBeInTheDocument();

    await vi.runAllTimersAsync();

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd]) => cmd === "agent_interaction_list_subagents"),
      ).toBe(true);
    });
    expect(await view.findByText("Reviewer")).toBeInTheDocument();
  });

  it("Agent 设置页卸载后取消自定义 Agent 目录延迟挂载", async () => {
    let rafCallback: FrameRequestCallback | null = null;
    const cancelAnimationFrame = vi.fn();
    vi.stubGlobal("requestAnimationFrame", vi.fn((callback: FrameRequestCallback) => {
      rafCallback = callback;
      return 15;
    }));
    vi.stubGlobal("cancelAnimationFrame", cancelAnimationFrame);
    mockInvoke.mockClear();

    const view = await renderSettings("/settings?tab=agent");
    view.unmount();

    expect(cancelAnimationFrame).toHaveBeenCalledWith(15);
    rafCallback?.(0);
    await Promise.resolve();

    expect(
      mockInvoke.mock.calls.some(([cmd]) => cmd === "agent_interaction_list_subagents"),
    ).toBe(false);
  });

  it("Agent 设置页可以保存 subagent 模式开关", async () => {
    const view = await renderSettings("/settings?tab=agent");
    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd]) => cmd === "agent_interaction_get_settings"),
      ).toBe(true);
    });
    expect(await view.findByText("Reviewer")).toBeInTheDocument();

    const subagentCardToggle = () =>
      view.getByRole("button", { name: /Subagent 详细配置/ });
    const subagentGroup = () => view.getByRole("radiogroup", { name: "Subagent 模式" });
    const codexGroup = () => view.queryByRole("radiogroup", { name: "Codex Subagent" });

    expect(subagentCardToggle()).toHaveAttribute("aria-expanded", "false");
    expect(subagentCardToggle()).toHaveAttribute("aria-disabled", "true");
    expect(codexGroup()).toBeNull();
    expect(view.queryByRole("radiogroup", { name: "Claude 转发子代理文本" })).toBeNull();

    await fireEvent.click(within(subagentGroup()).getByRole("radio", { name: "开启" }));

    await waitFor(() => {
      expect(lastInvokeInput("agent_interaction_set_settings")).toMatchObject({
        settings: {
          subagentMode: {
            enabled: true,
            codex: { enabled: true },
            claude: {
              enabled: true,
              forwardSubagentText: true,
              agentProgressSummaries: true,
            },
          },
        },
      });
    });
    await waitFor(() => {
      expect(within(subagentGroup()).getByRole("radio", { name: "开启" })).toHaveAttribute(
        "aria-checked",
        "true",
      );
      expect(subagentCardToggle()).toHaveAttribute("aria-expanded", "true");
      expect(subagentCardToggle()).toHaveAttribute("aria-label", "收起 Subagent 详细配置");
      expect(within(codexGroup() as HTMLElement).getByRole("radio", { name: "关闭" })).toBeEnabled();
    });

    await fireEvent.click(subagentCardToggle());
    expect(subagentCardToggle()).toHaveAttribute("aria-expanded", "false");
    expect(codexGroup()).toBeNull();

    await fireEvent.click(within(subagentGroup()).getByRole("radio", { name: "开启" }));
    expect(subagentCardToggle()).toHaveAttribute("aria-expanded", "false");
    expect(codexGroup()).toBeNull();

    await fireEvent.click(subagentCardToggle());
    expect(await view.findByRole("radiogroup", { name: "Codex Subagent" })).toBeInTheDocument();

    await fireEvent.click(within(codexGroup() as HTMLElement).getByRole("radio", { name: "关闭" }));

    await waitFor(() => {
      expect(lastInvokeInput("agent_interaction_set_settings")).toMatchObject({
        settings: {
          subagentMode: {
            enabled: true,
            codex: { enabled: false },
          },
        },
      });
    });

    const claudeTextGroup = view.getByRole("radiogroup", { name: "Claude 转发子代理文本" });
    await fireEvent.click(within(claudeTextGroup).getByRole("radio", { name: "关闭" }));

    await waitFor(() => {
      expect(lastInvokeInput("agent_interaction_set_settings")).toMatchObject({
        settings: {
          subagentMode: {
            enabled: true,
            claude: {
              enabled: true,
              forwardSubagentText: false,
              agentProgressSummaries: true,
            },
          },
        },
      });
    });

    await fireEvent.click(within(subagentGroup()).getByRole("radio", { name: "关闭" }));

    await waitFor(() => {
      expect(lastInvokeInput("agent_interaction_set_settings")).toMatchObject({
        settings: {
          subagentMode: {
            enabled: false,
            codex: { enabled: false },
            claude: {
              enabled: true,
              forwardSubagentText: false,
              agentProgressSummaries: true,
            },
          },
        },
      });
    });
    await waitFor(() => {
      expect(subagentCardToggle()).toHaveAttribute("aria-expanded", "false");
      expect(subagentCardToggle()).toHaveAttribute("aria-disabled", "true");
      expect(codexGroup()).toBeNull();
    });
  });

  it("项目设置页不再显示 Codex 项目默认高级字段", async () => {
    const view = await renderSettings("/settings?tab=project");

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === PROJECT_GET_SETTINGS_COMMAND)).toBe(true);
    });

    expect(view.queryByText("Codex 项目默认高级字段")).not.toBeInTheDocument();
    expect(view.queryByText("Codex 高级字段")).not.toBeInTheDocument();
  });

  it("项目设置页卸载后不保存迟到的目录选择结果", async () => {
    const originalInvoke = vi.mocked(mockInvoke).getMockImplementation();
    let resolvePickedFolder: (path: string | null) => void;
    vi.mocked(mockInvoke).mockImplementation((cmd: string, args: Record<string, unknown> = {}) => {
      if (cmd === TAURI_PLUGIN_DIALOG_OPEN_COMMAND) {
        return new Promise((resolve) => {
          resolvePickedFolder = resolve;
        });
      }
      return originalInvoke?.(cmd, args) ?? Promise.resolve(null);
    });
    try {
      const view = await renderSettings("/settings?tab=project");
      await waitFor(() => {
        expect(mockInvoke.mock.calls.some(([cmd]) => cmd === PROJECT_GET_SETTINGS_COMMAND)).toBe(true);
      });
      mockInvoke.mockClear();

      await waitFor(() => {
        expect(view.getAllByRole("button", { name: "选择" })[0]).toBeEnabled();
      });
      await fireEvent.click(view.getAllByRole("button", { name: "选择" })[0]);
      await waitFor(() => {
        expect(typeof resolvePickedFolder).toBe("function");
      });
      view.unmount();
      resolvePickedFolder!("C:\\late-picked");
      await Promise.resolve();
      await Promise.resolve();

      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === PROJECT_SET_SETTINGS_COMMAND)).toBe(false);
    } finally {
      vi.mocked(mockInvoke).mockImplementation(originalInvoke);
    }
  });

  it("新对话建议生成来源可切换到当前 Provider", async () => {
    const view = await renderSettings("/settings?tab=assistant");

    await fireEvent.click(view.getByRole("radio", { name: "当前 Provider" }));

    await waitFor(() => {
      expect(lastInvokeInput("conversation_suggestions_set_settings")).toMatchObject({
        settings: { source: "provider" },
      });
    });
  });

  it("辅助模型密钥不回显，空值保存保留已有密钥", async () => {
    const view = await renderSettings("/settings?tab=assistant");

    const apiKeyInput = await view.findByPlaceholderText("已保存，留空保留现有值") as HTMLInputElement;
    expect(apiKeyInput.value).toBe("");
    expect(view.getByText("密钥已保存")).toBeInTheDocument();

    await fireEvent.click(view.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(lastInvokeInput(ASSISTANT_AI_SET_CONFIG_COMMAND)).toMatchObject({
        config: { apiKey: null },
      });
      expect(lastInvokeInput(ASSISTANT_AI_SET_CONFIG_COMMAND)?.config).not.toMatchObject({
        clearApiKey: true,
      });
    });
    expect(view.getByText("密钥已保存")).toBeInTheDocument();
  });

  it("辅助模型可保存新密钥并显式清除", async () => {
    const view = await renderSettings("/settings?tab=assistant");
    const apiKeyInput = await view.findByPlaceholderText("已保存，留空保留现有值") as HTMLInputElement;

    await fireEvent.update(apiKeyInput, "sk-new");
    await fireEvent.click(view.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(lastInvokeInput(ASSISTANT_AI_SET_CONFIG_COMMAND)).toMatchObject({
        config: { apiKey: "sk-new" },
      });
    });

    await fireEvent.click(view.getByRole("button", { name: "清除" }));

    await waitFor(() => {
      expect(lastInvokeInput(ASSISTANT_AI_SET_CONFIG_COMMAND)).toMatchObject({
        config: { clearApiKey: true },
      });
      expect(view.getByText("未保存密钥")).toBeInTheDocument();
    });
  });

  it("辅助模型设置页卸载后不重新加载迟到的保存结果", async () => {
    const originalInvoke = vi.mocked(mockInvoke).getMockImplementation();
    let resolveSave: (() => void) | undefined;
    vi.mocked(mockInvoke).mockImplementation((cmd: string, args: Record<string, unknown> = {}) => {
      if (cmd === ASSISTANT_AI_SET_CONFIG_COMMAND) {
        return new Promise((resolve) => {
          resolveSave = () => resolve(undefined);
        });
      }
      return originalInvoke?.(cmd, args) ?? Promise.resolve(null);
    });

    try {
      const view = await renderSettings("/settings?tab=assistant");
      await view.findByPlaceholderText("已保存，留空保留现有值");
      mockInvoke.mockClear();

      await fireEvent.click(view.getByRole("button", { name: "保存" }));
      await waitFor(() => {
        expect(mockInvoke.mock.calls.some(([cmd]) => cmd === ASSISTANT_AI_SET_CONFIG_COMMAND))
          .toBe(true);
        expect(typeof resolveSave).toBe("function");
      });

      view.unmount();
      resolveSave?.();
      await Promise.resolve();
      await Promise.resolve();

      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === ASSISTANT_AI_GET_CONFIG_COMMAND))
        .toBe(false);
    } finally {
      vi.mocked(mockInvoke).mockImplementation(originalInvoke);
    }
  });

  it("项目设置页 GitHub 绑定流程冒烟", async () => {
    vi.useFakeTimers();
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    setMockGitHubPollSequence([
      {
        status: "authorized",
        intervalSeconds: 1,
        bindingStatus: {
          state: "bound",
          clientIdConfigured: true,
          clientIdSource: "bundled",
          binding: {
            login: "octocat",
            avatarUrl: null,
            boundAt: 1,
            scopes: ["repo", "read:user"],
            clientIdSource: "bundled",
          },
        },
        error: null,
      },
    ]);

    const view = await renderSettings("/settings?tab=project");
    await fireEvent.click(view.getByRole("button", { name: "绑定 GitHub" }));
    await waitFor(() => {
      expect(
        view.getByRole("button", { name: "复制设备码并打开浏览器" }),
      ).toBeInTheDocument();
    });

    await fireEvent.click(
      view.getByRole("button", { name: "复制设备码并打开浏览器" }),
    );

    await vi.advanceTimersByTimeAsync(1000);

    await waitFor(() => {
      expect(view.getByText(/已绑定：\s*octocat/)).toBeInTheDocument();
      expect(view.queryByText(/权限：/)).not.toBeInTheDocument();
    });
    expect(writeText).toHaveBeenCalledWith("ABCD-EFGH");
    expect(
      mockInvoke.mock.calls.some(([cmd]) => cmd === GITHUB_START_DEVICE_FLOW_COMMAND),
    ).toBe(true);
    expect(
      mockInvoke.mock.calls.some(([cmd]) => cmd === GITHUB_POLL_DEVICE_FLOW_COMMAND),
    ).toBe(true);
    expect(
      mockInvoke.mock.calls.some(([cmd, args]) =>
        cmd === SYSTEM_OPEN_URL_COMMAND &&
        typeof args === "object" &&
        args !== null &&
        "url" in args &&
        args.url === "https://github.com/login/device"
      ),
    ).toBe(true);
  });
});
