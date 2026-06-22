import { fireEvent, render, waitFor } from "@testing-library/vue";
import { describe, expect, it, vi } from "vitest";
import { toDataURL } from "qrcode";
import { REMOTE_CONTROL_START_PAIRING_COMMAND } from "@lilia/contracts";
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
      expect(view.getByText("未启用")).toBeInTheDocument();
    });

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
});
