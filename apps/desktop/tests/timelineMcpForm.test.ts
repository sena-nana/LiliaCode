import { describe, expect, it } from "vitest";
import {
  timelineMcpCanSubmit,
  timelineMcpContentFromForm,
  timelineMcpFieldInputType,
  timelineMcpFieldsForSchema,
  timelineMcpInitialValues,
  timelineMcpMultiSelected,
  timelineMcpToggleMultiValue,
} from "../src/components/chat/timelineMcpForm";

describe("timeline MCP form helpers", () => {
  it("derives form fields from JSON schema properties", () => {
    const fields = timelineMcpFieldsForSchema({
      required: ["project", "count"],
      properties: {
        project: {
          title: "Project",
          description: "Project key",
          type: "string",
          enum: ["lilia", "sakura"],
          default: "lilia",
        },
        count: {
          title: "Count",
          type: "integer",
        },
        tags: {
          title: "Tags",
          type: "array",
          items: {
            anyOf: [
              { const: "ui", title: "UI" },
              { const: "backend", title: "Backend" },
            ],
          },
        },
      },
    });

    expect(fields).toMatchObject([
      {
        key: "project",
        label: "Project",
        description: "Project key",
        type: "string",
        required: true,
        defaultValue: "lilia",
        options: [
          { value: "lilia", label: "lilia" },
          { value: "sakura", label: "sakura" },
        ],
      },
      {
        key: "count",
        label: "Count",
        type: "integer",
        required: true,
        multi: false,
      },
      {
        key: "tags",
        label: "Tags",
        type: "array",
        multi: true,
        options: [
          { value: "ui", label: "UI" },
          { value: "backend", label: "Backend" },
        ],
      },
    ]);
  });

  it("initializes values and validates required fields", () => {
    const fields = timelineMcpFieldsForSchema({
      required: ["enabled", "name", "tags"],
      properties: {
        enabled: { type: "boolean" },
        name: { type: "string" },
        tags: { type: "array" },
      },
    });
    const values = timelineMcpInitialValues(fields);

    expect(values).toEqual({
      enabled: false,
      name: "",
      tags: [],
    });
    expect(timelineMcpCanSubmit({
      fields,
      jsonText: "{}",
      mode: "form",
      values,
    })).toBe(false);
    expect(timelineMcpCanSubmit({
      fields,
      jsonText: "{}",
      mode: "form",
      values: { ...values, name: "Lilia", tags: ["ui"] },
    })).toBe(true);
  });

  it("validates raw JSON when the schema has no form fields", () => {
    expect(timelineMcpCanSubmit({
      fields: [],
      jsonText: "{",
      mode: "form",
      values: {},
    })).toBe(false);
    expect(timelineMcpCanSubmit({
      fields: [],
      jsonText: "{\"project\":\"lilia\"}",
      mode: "form",
      values: {},
    })).toBe(true);
    expect(timelineMcpCanSubmit({
      fields: [],
      jsonText: "{",
      mode: "text",
      values: {},
    })).toBe(true);
  });

  it("builds MCP content from typed values and raw JSON", () => {
    const fields = timelineMcpFieldsForSchema({
      required: ["name"],
      properties: {
        count: { type: "integer" },
        enabled: { type: "boolean" },
        name: { type: "string" },
        ratio: { type: "number" },
        tags: { type: "array" },
      },
    });

    expect(timelineMcpContentFromForm({
      fields,
      jsonText: "{}",
      values: {
        count: "3.8",
        enabled: true,
        name: "Lilia",
        ratio: "1.5",
        tags: ["ui"],
      },
    })).toEqual({
      count: 3,
      enabled: true,
      name: "Lilia",
      ratio: 1.5,
      tags: ["ui"],
    });
    expect(timelineMcpContentFromForm({
      fields: [],
      jsonText: "{\"project\":\"lilia\"}",
      values: {},
    })).toEqual({ project: "lilia" });
  });

  it("toggles multi-select values without mutating the source object", () => {
    const original = { tags: ["ui"] };
    const added = timelineMcpToggleMultiValue(original, "tags", "backend");
    const removed = timelineMcpToggleMultiValue(added, "tags", "ui");

    expect(original).toEqual({ tags: ["ui"] });
    expect(added).toEqual({ tags: ["ui", "backend"] });
    expect(removed).toEqual({ tags: ["backend"] });
    expect(timelineMcpMultiSelected(added, "tags", "backend")).toBe(true);
    expect(timelineMcpFieldInputType("integer")).toBe("number");
    expect(timelineMcpFieldInputType("string")).toBe("text");
  });
});
