// Shared tiny helpers for runner-safe .mjs modules.
// Keep this file dependency-free: agent-runner imports contracts directly in Node.

export function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function readRecord(value) {
  return isRecord(value) ? value : {};
}

export function pick(record, keys) {
  for (const key of keys) {
    const value = record[key];
    if (value !== undefined && value !== null && value !== "") return value;
  }
  return undefined;
}

export function stringOrNull(value) {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return null;
}

export function shortText(value, max) {
  const text = stringOrNull(value);
  if (!text) return "";
  return text.length > max ? `${text.slice(0, max)}...` : text;
}

export function stringifyInline(value) {
  if (typeof value === "string") return value.trim();
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) {
    return value.map((item) => stringifyInline(item)).filter(Boolean).join(" ").trim();
  }
  if (isRecord(value)) {
    return readFirstString(value, [
      "text", "title", "summary", "content", "message",
      "name", "path", "filePath", "query", "command",
    ], 600);
  }
  return "";
}

export function compactLine(value, max = 600) {
  const text = stringifyInline(value).replace(/\s+/g, " ").trim();
  return text ? shortText(text, max) : "";
}

export function readFirstString(payload, keys, max = 600) {
  const record = readRecord(payload);
  for (const key of keys) {
    const text = compactLine(record[key], max);
    if (text) return text;
  }
  return "";
}

export function readFirstText(payload, keys, max = 600) {
  const record = readRecord(payload);
  for (const key of keys) {
    const text = shortText(stringOrNull(record[key])?.trim() ?? "", max);
    if (text) return text;
  }
  return "";
}

export function pickString(record, keys, max = 600) {
  return readFirstString(record, keys, max);
}

export function pickNumber(record, keys) {
  const source = readRecord(record);
  for (const key of keys) {
    const value = source[key];
    if (typeof value === "number" && Number.isFinite(value)) return value;
  }
  return undefined;
}

export function parseRecordJson(value) {
  if (isRecord(value)) return value;
  if (typeof value !== "string" || !value.trim()) return null;
  try {
    const parsed = JSON.parse(value);
    return isRecord(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

export function readArrayRecords(value) {
  return (Array.isArray(value) ? value : []).filter(isRecord);
}
