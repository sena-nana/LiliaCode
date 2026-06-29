import { fireEvent, render, waitFor } from "@testing-library/vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { APP_RESTART_COMMAND } from "@lilia/contracts";
import AppUpdateHost from "../src/components/AppUpdateHost.vue";
import { resetDesktopAppUpdaterForTest } from "../src/composables/useDesktopAppUpdater";
import { mockInvoke } from "./tauriMock";

const updaterCheck = vi.fn();

vi.mock("@tauri-apps/plugin-updater", () => ({
  check: (...args: unknown[]) => updaterCheck(...args),
}));

function createUpdate(version = "1.0.0-beta.2") {
  return {
    currentVersion: "1.0.0-beta.1",
    version,
    date: "2026-06-29T00:00:00Z",
    body: "Release notes",
    rawJson: {},
    downloadAndInstall: vi.fn(async (onEvent?: (event: unknown) => void) => {
      onEvent?.({ event: "Started", data: { contentLength: 100 } });
      onEvent?.({ event: "Progress", data: { chunkLength: 40 } });
      onEvent?.({ event: "Finished" });
    }),
  };
}

function renderHost(options: {
  isProduction?: boolean;
  windowLabel?: string;
} = {}) {
  return render(AppUpdateHost, {
    props: {
      isProduction: options.isProduction ?? true,
      windowLabel: options.windowLabel ?? "main",
    },
  });
}

describe("AppUpdateHost", () => {
  beforeEach(() => {
    resetDesktopAppUpdaterForTest();
    updaterCheck.mockReset();
    mockInvoke.mockClear();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("does not show a dialog when no update is available", async () => {
    updaterCheck.mockResolvedValue(null);

    const view = renderHost();

    await waitFor(() => expect(updaterCheck).toHaveBeenCalledTimes(1));
    expect(view.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("shows an available update only once per version after dismissal", async () => {
    updaterCheck.mockResolvedValue(createUpdate("1.0.0-beta.2"));

    const first = renderHost();

    expect(await first.findByRole("dialog", { name: "发现 LiliaCode 1.0.0-beta.2" }))
      .toBeInTheDocument();
    await fireEvent.click(first.getByRole("button", { name: "暂不更新" }));
    first.unmount();

    const second = renderHost();

    await waitFor(() => expect(updaterCheck).toHaveBeenCalledTimes(2));
    expect(second.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("downloads, installs, and restarts after confirmation", async () => {
    const update = createUpdate();
    updaterCheck.mockResolvedValue(update);

    const view = renderHost();

    await fireEvent.click(await view.findByRole("button", { name: "更新并重启" }));

    await waitFor(() => {
      expect(update.downloadAndInstall).toHaveBeenCalledTimes(1);
      expect(mockInvoke).toHaveBeenCalledWith(APP_RESTART_COMMAND, {}, undefined);
    });
    expect(view.getByRole("dialog")).toHaveTextContent("重启");
  });

  it("allows retry when installation fails", async () => {
    const update = createUpdate();
    update.downloadAndInstall
      .mockRejectedValueOnce(new Error("network"))
      .mockResolvedValueOnce(undefined);
    updaterCheck.mockResolvedValue(update);

    const view = renderHost();

    await fireEvent.click(await view.findByRole("button", { name: "更新并重启" }));
    expect(await view.findByRole("dialog", { name: "更新失败" })).toHaveTextContent(
      "更新没有完成",
    );

    await fireEvent.click(view.getByRole("button", { name: "重试" }));

    await waitFor(() => {
      expect(update.downloadAndInstall).toHaveBeenCalledTimes(2);
      expect(mockInvoke).toHaveBeenCalledWith(APP_RESTART_COMMAND, {}, undefined);
    });
  });

  it("does not auto-check outside the main production window", async () => {
    renderHost({ windowLabel: "popup-task", isProduction: true });
    renderHost({ windowLabel: "main", isProduction: false });

    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(updaterCheck).not.toHaveBeenCalled();
  });
});
