import type { AgentTimelineDisplay, AgentTimelineDisplayDetail } from "../timeline";

interface DisplayContext {
  projectCwd: string;
  projectCwdForCompare: string;
}

export function createDisplayContext(projectCwd: string | null | undefined): DisplayContext | null {
  const normalized = normalizePath(projectCwd);
  if (!normalized || !isAbsolutePath(normalized) || isUrl(normalized)) return null;
  const caseInsensitive = isWindowsPath(normalized);
  return {
    projectCwd: normalized,
    projectCwdForCompare: normalizeForCompare(normalized, caseInsensitive),
  };
}

export function applyDisplayContext(
  display: AgentTimelineDisplay,
  context: DisplayContext | null,
): AgentTimelineDisplay {
  if (!context) return display;
  return {
    ...display,
    object: shortenProjectPath(display.object, context) ?? display.object,
    preview: shortenProjectPath(display.preview, context) ?? display.preview,
    details: display.details?.map((detail) => applyDisplayContextToDetail(detail, context)),
  };
}

function applyDisplayContextToDetail(
  detail: AgentTimelineDisplayDetail,
  context: DisplayContext,
): AgentTimelineDisplayDetail {
  switch (detail.type) {
    case "fields":
      return {
        ...detail,
        fields: detail.fields.map((field) => ({
          ...field,
          value: shouldShortenField(field.label)
            ? shortenProjectPath(field.value, context) ?? field.value
            : field.value,
        })),
      };
    case "list":
      return {
        ...detail,
        items: detail.items.map((item) => ({
          ...item,
          text: shortenPathsInText(item.text, context),
        })),
      };
    default:
      return detail;
  }
}

function shouldShortenField(label: string): boolean {
  const normalized = label.trim().toLowerCase();
  return normalized === "path" || normalized === "cwd";
}

function shortenPathsInText(text: string, context: DisplayContext): string {
  const shortened = shortenProjectPath(text, context) ?? text;
  if (shortened !== text) return shortened;
  const prefix = `${context.projectCwd}/`;
  const index = normalizeForCompare(text.replace(/\\/g, "/"), isWindowsPath(context.projectCwd))
    .indexOf(`${context.projectCwdForCompare}/`);
  if (index < 0) return text;
  return `${text.slice(0, index)}${text.slice(index + prefix.length)}`.replace(/\\/g, "/");
}

function shortenProjectPath(
  value: string | null | undefined,
  context: DisplayContext | null,
): string | null | undefined {
  if (!value || !context) return value;
  const normalized = normalizePath(value);
  if (!normalized || isUrl(normalized) || !isAbsolutePath(normalized)) return value;
  const normalizedForCompare = normalizeForCompare(
    normalized,
    isWindowsPath(normalized) && isWindowsPath(context.projectCwd),
  );
  if (normalizedForCompare === context.projectCwdForCompare) return ".";
  const prefix = `${context.projectCwdForCompare}/`;
  if (!normalizedForCompare.startsWith(prefix)) return value;
  const relative = normalized.slice(context.projectCwd.length);
  return trimLeadingSeparators(relative) || ".";
}

function normalizePath(value: string | null | undefined): string {
  return (value ?? "").trim().replace(/\\/g, "/").replace(/\/+$/g, "");
}

function normalizeForCompare(value: string, caseInsensitive: boolean): string {
  return caseInsensitive ? value.toLowerCase() : value;
}

function isAbsolutePath(value: string): boolean {
  return /^[a-zA-Z]:\//.test(value) || value.startsWith("/");
}

function isWindowsPath(value: string): boolean {
  return /^[a-zA-Z]:\//.test(value);
}

function isUrl(value: string): boolean {
  return /^[a-zA-Z][a-zA-Z\d+.-]*:\/\//.test(value);
}

function trimLeadingSeparators(value: string): string {
  return value.replace(/^\/+/, "");
}
