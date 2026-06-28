function htmlToPlainText(html: string): string {
  const template = document.createElement("template");
  template.innerHTML = html;
  return template.content.textContent ?? "";
}

export function pastedImageFiles(event: ClipboardEvent): File[] {
  const items = Array.from(event.clipboardData?.items ?? []);
  return items
    .filter((item) => item.kind === "file" && item.type.startsWith("image/"))
    .map((item) => item.getAsFile())
    .filter((file): file is File => file !== null);
}

export function pasteHasFileItems(event: ClipboardEvent): boolean {
  const data = event.clipboardData;
  if (!data) return false;
  return Array.from(data.items).some((item) => item.kind === "file") || data.files.length > 0;
}

export function pastedPlainText(event: ClipboardEvent): string {
  const data = event.clipboardData;
  if (!data) return "";
  return data.getData("text/plain") || htmlToPlainText(data.getData("text/html"));
}

