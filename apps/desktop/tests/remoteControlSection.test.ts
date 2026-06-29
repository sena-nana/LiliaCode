import { fireEvent, render, waitFor } from "@testing-library/vue";
import { describe, expect, it, vi } from "vitest";
import { toDataURL } from "qrcode";
import {
  REMOTE_CONTROL_PAIR_DEVICE_COMMAND,
  REMOTE_CONTROL_START_PAIRING_COMMAND,
} from "@lilia/contracts";
import RemoteControlSection from "../src/pages/settings/RemoteControlSection.vue";
import { mockInvoke } from "./tauriMock";

vi.mock("qrcode", () => ({
  toDataURL: vi.fn(async (value: string) => `data:image/png;base64,${btoa(value)}`),
}));

describe("RemoteControlSection", () => {
  it("生成包含 bridge URL 的 Android 配对二维码", async () => {
    const view = render(RemoteControlSection);

    await waitFor(() => {
      expect(view.getByRole("heading", { level: 2, name: "Android 远控" })).toBeInTheDocument();
    });
    expect(view.getByRole("switch", { name: "远控服务" })).not.toBeChecked();
    expect(view.getByRole("switch", { name: "远控期间保持电脑唤醒" })).toBeChecked();

    await fireEvent.click(view.getByRole("button", { name: "生成二维码" }));

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === REMOTE_CONTROL_START_PAIRING_COMMAND)).toBe(true);
      expect(view.getByAltText("Lilia Android pairing QR code")).toBeInTheDocument();
    });

    const generatedUri = vi.mocked(toDataURL).mock.calls.at(-1)?.[0] as string | undefined;
    expect(generatedUri).toMatch(/^lilia-remote:\/\/pair\?/);
    expect(generatedUri).toContain("ticket=mock-ticket");
    expect(generatedUri).toContain("bridge=http%3A%2F%2F127.0.0.1%3A41478");
    expect(view.getByText(generatedUri!)).toBeInTheDocument();
  });

  it("轮询刷新配对后的连接状态和可信设备", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-22T00:00:00Z"));
    const view = render(RemoteControlSection);
    try {
      await waitFor(() => {
        expect(view.getByRole("switch", { name: "远控服务" })).not.toBeChecked();
      });

      await fireEvent.click(view.getByRole("button", { name: "生成二维码" }));

      await waitFor(() => {
        expect(view.getByAltText("Lilia Android pairing QR code")).toBeInTheDocument();
      });

      await mockInvoke(REMOTE_CONTROL_PAIR_DEVICE_COMMAND, {
        input: {
          ticketId: "mock-ticket",
          challenge: "mock-challenge",
          deviceName: "Pixel",
          androidEndpoint: { endpointId: "mock-android-endpoint" },
          protocolVersion: 1,
        },
      });
      await vi.advanceTimersByTimeAsync(5_000);

      await waitFor(() => {
        expect(view.getByRole("switch", { name: "远控服务" })).toBeChecked();
        expect(view.getByText("Pixel")).toBeInTheDocument();
        expect(view.queryByAltText("Lilia Android pairing QR code")).not.toBeInTheDocument();
      });
    } finally {
      view.unmount();
      vi.useRealTimers();
    }
  });

  it("关闭远控后隐藏配对二维码", async () => {
    const view = render(RemoteControlSection);

    await waitFor(() => {
      expect(view.getByRole("switch", { name: "远控服务" })).not.toBeChecked();
    });

    await fireEvent.click(view.getByRole("button", { name: "生成二维码" }));

    await waitFor(() => {
      expect(view.getByAltText("Lilia Android pairing QR code")).toBeInTheDocument();
    });
    expect(view.getByRole("switch", { name: "远控服务" })).toBeChecked();

    await fireEvent.click(view.getByRole("switch", { name: "远控服务" }));

    await waitFor(() => {
      expect(view.getByRole("switch", { name: "远控服务" })).not.toBeChecked();
      expect(view.queryByAltText("Lilia Android pairing QR code")).not.toBeInTheDocument();
    });
  });
});

