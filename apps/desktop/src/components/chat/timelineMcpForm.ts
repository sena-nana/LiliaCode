export interface TimelineMcpFormField {
  defaultValue: unknown;
  description: string;
  key: string;
  label: string;
  multi: boolean;
  options: Array<{ value: string; label: string }>;
  required: boolean;
  type: string;
}

export type TimelineMcpFormValues = Record<string, unknown>;

function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function timelineMcpEnumOptions(
  field: Record<string, unknown>,
): Array<{ value: string; label: string }> {
  const rawEnum = Array.isArray(field.enum) ? field.enum : null;
  if (rawEnum) {
    return rawEnum
      .map((value) => typeof value === "string" ? { value, label: value } : null)
      .filter((value): value is { value: string; label: string } => value !== null);
  }
  const oneOf = Array.isArray(field.oneOf)
    ? field.oneOf
    : Array.isArray((field.items as Record<string, unknown> | undefined)?.anyOf)
      ? (field.items as Record<string, unknown>).anyOf as unknown[]
      : null;
  if (!oneOf) return [];
  return oneOf
    .map((item) => {
      if (!item || typeof item !== "object" || Array.isArray(item)) return null;
      const row = item as Record<string, unknown>;
      const value = stringValue(row.const);
      if (!value) return null;
      return { value, label: stringValue(row.title) || value };
    })
    .filter((value): value is { value: string; label: string } => value !== null);
}

export function timelineMcpFieldsForSchema(schema: unknown): TimelineMcpFormField[] {
  if (!schema || typeof schema !== "object" || Array.isArray(schema)) return [];
  const row = schema as Record<string, unknown>;
  const properties = row.properties;
  if (!properties || typeof properties !== "object" || Array.isArray(properties)) return [];
  const required = Array.isArray(row.required)
    ? new Set(row.required.filter((item): item is string => typeof item === "string"))
    : new Set<string>();
  return Object.entries(properties as Record<string, unknown>).map(([key, value]) => {
    const field = value && typeof value === "object" && !Array.isArray(value)
      ? value as Record<string, unknown>
      : {};
    return {
      key,
      label: stringValue(field.title) || key,
      description: stringValue(field.description),
      type: stringValue(field.type) || "string",
      required: required.has(key),
      options: timelineMcpEnumOptions(field),
      multi: stringValue(field.type) === "array",
      defaultValue: field.default,
    };
  });
}

export function timelineMcpFieldInputType(type: string): string {
  if (type === "number" || type === "integer") return "number";
  return "text";
}

export function timelineMcpInitialValues(
  fields: readonly TimelineMcpFormField[],
): TimelineMcpFormValues {
  const values: TimelineMcpFormValues = {};
  for (const field of fields) {
    if (field.defaultValue !== undefined) {
      values[field.key] = field.defaultValue;
    } else if (field.type === "boolean") {
      values[field.key] = false;
    } else if (field.multi) {
      values[field.key] = [];
    } else {
      values[field.key] = "";
    }
  }
  return values;
}

export function timelineMcpMultiSelected(
  values: TimelineMcpFormValues,
  key: string,
  value: string,
): boolean {
  const current = values[key];
  return Array.isArray(current) && current.includes(value);
}

export function timelineMcpToggleMultiValue(
  values: TimelineMcpFormValues,
  key: string,
  value: string,
): TimelineMcpFormValues {
  const current = values[key];
  const selected = Array.isArray(current)
    ? current.filter((item): item is string => typeof item === "string")
    : [];
  return {
    ...values,
    [key]: selected.includes(value)
      ? selected.filter((item) => item !== value)
      : [...selected, value],
  };
}

export function timelineMcpCanSubmit(input: {
  fields: readonly TimelineMcpFormField[];
  jsonText: string;
  mode: string | null | undefined;
  values: TimelineMcpFormValues;
}): boolean {
  if (input.mode !== "form") return true;
  if (input.fields.length === 0) {
    try {
      JSON.parse(input.jsonText || "{}");
      return true;
    } catch {
      return false;
    }
  }
  return input.fields.every((field) => {
    if (!field.required) return true;
    const value = input.values[field.key];
    if (Array.isArray(value)) return value.length > 0;
    if (typeof value === "boolean") return true;
    return String(value ?? "").trim().length > 0;
  });
}

export function timelineMcpContentFromForm(input: {
  fields: readonly TimelineMcpFormField[];
  jsonText: string;
  values: TimelineMcpFormValues;
}): Record<string, unknown> {
  if (input.fields.length === 0) {
    const parsed = JSON.parse(input.jsonText || "{}");
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : {};
  }
  const content: Record<string, unknown> = {};
  for (const field of input.fields) {
    const raw = input.values[field.key];
    if (field.type === "number" || field.type === "integer") {
      const number = typeof raw === "number" ? raw : Number(raw);
      if (Number.isFinite(number)) content[field.key] = field.type === "integer"
        ? Math.trunc(number)
        : number;
      continue;
    }
    if (field.type === "boolean") {
      content[field.key] = raw === true;
      continue;
    }
    if (Array.isArray(raw)) {
      content[field.key] = raw;
      continue;
    }
    const text = String(raw ?? "");
    if (text || field.required) content[field.key] = text;
  }
  return content;
}

