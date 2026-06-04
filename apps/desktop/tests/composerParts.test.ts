import { describe, expect, it } from "vitest";
import type { ChatAttachment } from "@lilia/contracts";
import {
  attachmentPart,
  composerPartsLength,
  normalizeComposerParts,
  replaceComposerPartsRange,
  serializeComposerParts,
  splitComposerPartsAt,
  textPart,
} from "../src/components/chat/composerParts";

function attachment(input: Partial<ChatAttachment> = {}): ChatAttachment {
  return {
    id: input.id ?? "att-1",
    name: input.name ?? "README.md",
    path: input.path ?? "D:\\PROJECT\\workspace\\Lilia\\README.md",
    kind: input.kind ?? "file",
    size: input.size ?? 42,
    exists: input.exists ?? true,
    mime: input.mime ?? null,
    directory: input.directory ?? null,
  };
}

describe("composerParts", () => {
  it("合并相邻文本并保留空输入占位", () => {
    expect(normalizeComposerParts([textPart("a"), textPart("b")])).toMatchObject([
      { type: "text", text: "ab" },
    ]);

    expect(normalizeComposerParts([])).toMatchObject([
      { type: "text", text: "" },
    ]);
  });

  it("按文本和附件长度拆分 parts", () => {
    const file = attachment();
    const parts = [textPart("ab"), attachmentPart(file), textPart("cd")];

    const [before, after] = splitComposerPartsAt(parts, 3);

    expect(composerPartsLength(before)).toBe(3);
    expect(serializeComposerParts(before)).toContain("README.md");
    expect(serializeComposerParts(after)).toBe("cd");
  });

  it("替换中间区间并返回新光标", () => {
    const parts = [textPart("前  后")];
    const file = attachment({ name: "context.ts", path: "D:\\PROJECT\\context.ts" });

    const next = replaceComposerPartsRange(parts, 1, 3, [
      attachmentPart(file),
      textPart(" "),
    ]);

    expect(next.cursor).toBe(3);
    expect(serializeComposerParts(next.parts)).toBe(
      "前[文件引用: context.ts | D:\\PROJECT\\context.ts] 后",
    );
  });

  it("序列化文件、目录和图片引用文案", () => {
    const parts = [
      textPart("参考 "),
      attachmentPart(attachment({ name: "src", path: "D:\\PROJECT\\src", kind: "directory" })),
      textPart(" 和 "),
      attachmentPart(attachment({
        name: "shot.png",
        path: "D:\\PROJECT\\shot.png",
        mime: "image/png",
      })),
    ];

    expect(serializeComposerParts(parts)).toBe(
      "参考 [目录引用: src | D:\\PROJECT\\src] 和 [图片引用: shot.png | D:\\PROJECT\\shot.png]",
    );
  });
});
