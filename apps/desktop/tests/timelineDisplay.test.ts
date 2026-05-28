import { describe, expect, it } from "vitest";
import { deriveTimelineDisplay } from "@lilia/contracts";

function codeContent(
  details: ReturnType<typeof deriveTimelineDisplay>["details"],
  label: string,
): string {
  const detail = details?.find((item) => item.type === "code" && item.label === label);
  return detail?.type === "code" ? detail.content : "";
}

describe("timeline display derivation", () => {
  it("命令输出详情保留原始分行", () => {
    const display = deriveTimelineDisplay({
      kind: "command",
      status: "success",
      title: "yarn test",
      summary: "",
      payload: {
        command: "yarn test",
        output: "line one\nline two\nline three",
      },
    });

    expect(codeContent(display.details, "OUTPUT")).toBe("line one\nline two\nline three");
  });
});
