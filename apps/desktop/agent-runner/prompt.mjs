import { isRecord, stringOrNull } from "./utils.mjs";

export function normalizePromptAttachments(value) {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => {
      if (!isRecord(item)) return null;
      const path = stringOrNull(item.path)?.trim();
      if (!path) return null;
      return {
        path,
        name: stringOrNull(item.name) || path,
        kind: stringOrNull(item.kind) || "unknown",
        mime: stringOrNull(item.mime),
        size: typeof item.size === "number" && Number.isFinite(item.size) ? item.size : null,
        directory: isRecord(item.directory) ? item.directory : null,
      };
    })
    .filter(Boolean);
}

export function attachmentSizeLabel(size) {
  if (typeof size !== "number" || !Number.isFinite(size) || size < 0) return "unknown size";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${Math.round(size / 1024)} KB`;
  return `${Math.round(size / (1024 * 1024))} MB`;
}

export function attachmentDirectoryLabel(directory) {
  if (!directory) return "";
  const parts = [];
  if (typeof directory.fileCount === "number") parts.push(`${directory.fileCount} files`);
  if (typeof directory.directoryCount === "number") parts.push(`${directory.directoryCount} dirs`);
  if (directory.truncated === true) parts.push("truncated");
  return parts.join(", ");
}

export function buildPromptWithAttachments(prompt, attachments) {
  const normalized = normalizePromptAttachments(attachments);
  const content = typeof prompt === "string" ? prompt : "";
  if (normalized.length === 0) return content;
  const lines = [
    "用户随本轮消息附加的本地路径如下。不要假设已经读取了内容；需要时请使用可用工具按路径读取文件或遍历目录。",
    ...normalized.map((attachment, index) => {
      const meta = [
        attachment.kind,
        attachment.mime,
        attachmentSizeLabel(attachment.size),
        attachmentDirectoryLabel(attachment.directory),
      ].filter(Boolean).join(", ");
      return `${index + 1}. ${attachment.name}: ${attachment.path}${meta ? ` (${meta})` : ""}`;
    }),
  ];
  const trimmedContent = content.trim();
  return trimmedContent
    ? `${lines.join("\n")}\n\n用户消息：\n${trimmedContent}`
    : lines.join("\n");
}
