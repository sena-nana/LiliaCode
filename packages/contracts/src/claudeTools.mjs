// Claude 工具表 —— runner（agent-runner.mjs）和前端（timelineDisplay.ts）共享的
// 单一事实表。每个工具一条记录，含 lilia 的分类（kind）、input 摘要字段
// （summaryFields）、以及 timeline 渲染规则（display）。
//
// 改一处两边同步。新增工具：在 CLAUDE_TOOLS 加一条；未列入的工具自动落 CLAUDE_TOOL_DEFAULT。
//
// 这是 .mjs 而非 .ts 是因为 runner 由 Tauri 直接 `node agent-runner.mjs` 拉起，
// 不经过任何构建步骤；TS 端通过同目录 claudeTools.d.mts 拿到类型。

// ---------- helper ----------

function isRecord(value) {
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

function stringOrNull(value) {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return null;
}

function shortText(value, max) {
  const text = stringOrNull(value);
  if (!text) return "";
  return text.length > max ? `${text.slice(0, max)}...` : text;
}

function stringifyInline(value) {
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

export function compactLine(value, max) {
  const text = stringifyInline(value).replace(/\s+/g, " ").trim();
  return text ? shortText(text, max) : "";
}

export function readFirstString(payload, keys, max) {
  for (const key of keys) {
    const text = compactLine(payload[key], max);
    if (text) return text;
  }
  return "";
}

export function displayField(label, value) {
  const text = compactLine(value, 1200);
  return label && text ? { label, value: text } : null;
}

export function fieldsDetail(fields) {
  const items = fields.filter((field) => field !== null);
  return items.length ? { type: "fields", fields: items } : null;
}

export function codeDetail(label, content, language = "") {
  let text = stringOrNull(content);
  if (!text && (Array.isArray(content) || isRecord(content))) {
    try {
      text = JSON.stringify(content, null, 2);
    } catch {
      text = String(content);
    }
  }
  if (!text || !text.trim()) return null;
  return {
    type: "code",
    label: label || null,
    content: shortText(text.trim(), 6000),
    language: language || null,
  };
}

export function markdownDetail(content, tone = "default", singleLine = false) {
  const text = stringOrNull(content);
  if (!text || !text.trim()) return null;
  return {
    type: "markdown",
    content: shortText(text.trim(), 6000),
    tone,
    singleLine,
  };
}

export function listDetail(items, ordered = false) {
  const normalized = (Array.isArray(items) ? items : [])
    .map((item) => {
      if (typeof item === "string") return { text: compactLine(item, 1200) };
      if (!isRecord(item)) return null;
      const text = readFirstString(item, ["text", "content", "title", "summary"], 1200);
      if (!text) return null;
      const status = String(item.status ?? "").toLowerCase();
      const completed = item.completed === true || item.done === true || status === "completed";
      const tone = completed
        ? "success"
        : status === "failed" || status === "error"
          ? "error"
          : "default";
      return { text, tone };
    })
    .filter((item) => item !== null && Boolean(item.text));
  return normalized.length ? { type: "list", items: normalized, ordered } : null;
}

export function readTodoItems(payload) {
  const input = readRecord(payload.input);
  const raw =
    (Array.isArray(payload.items) && payload.items) ||
    (Array.isArray(payload.todos) && payload.todos) ||
    (Array.isArray(input.items) && input.items) ||
    (Array.isArray(input.todos) && input.todos) ||
    [];
  return raw
    .map((item) => {
      if (typeof item === "string") return { text: item, completed: false };
      if (!isRecord(item)) return null;
      const text = readFirstString(item, ["text", "content", "title", "description"], 1200);
      if (!text) return null;
      const status = String(item.status ?? "").toLowerCase();
      return {
        text,
        completed: item.completed === true || item.done === true || status === "completed",
        status,
      };
    })
    .filter((item) => item !== null);
}

// ---------- 工具表 ----------

const FILE_FIELDS = ["file_path", "path"];

export const CLAUDE_TOOLS = {
  Bash: {
    kind: "command",
    summaryFields: ["command", "description"],
    display: {
      action: "运行",
      icon: "terminal",
      bucket: "command",
      unit: "条命令",
      extractObject: (input) =>
        compactLine(pick(input, ["command", "description"]), 1200),
      buildDetails: (input, payload) => [
        fieldsDetail([
          displayField("cwd", pick(payload, ["cwd"])),
          displayField("exit", pick(payload, ["exitCode"])),
        ]),
        codeDetail("COMMAND", pick(input, ["command"]), "shell"),
        codeDetail("OUTPUT", pick(payload, ["output"])),
      ],
    },
  },
  Read: {
    kind: "file_read",
    summaryFields: FILE_FIELDS,
    display: {
      action: "读取",
      icon: "book-open",
      bucket: "file",
      unit: "个文件",
      extractObject: (input) => compactLine(pick(input, FILE_FIELDS), 1200),
      buildDetails: (input) => [
        fieldsDetail([
          displayField("文件", pick(input, FILE_FIELDS)),
          displayField("offset", pick(input, ["offset"])),
          displayField("limit", pick(input, ["limit"])),
        ]),
      ],
    },
  },
  Edit: {
    kind: "file_change",
    summaryFields: FILE_FIELDS,
    display: {
      action: "修改",
      icon: "file-pen",
      bucket: "file",
      unit: "个文件",
      extractObject: (input) => compactLine(pick(input, FILE_FIELDS), 1200),
      buildDetails: (input) => [
        fieldsDetail([displayField("文件", pick(input, FILE_FIELDS))]),
      ],
    },
  },
  MultiEdit: {
    kind: "file_change",
    summaryFields: FILE_FIELDS,
    display: {
      action: "批量修改",
      icon: "file-pen",
      bucket: "file",
      unit: "个文件",
      extractObject: (input) => compactLine(pick(input, FILE_FIELDS), 1200),
      buildDetails: (input) => {
        const edits = Array.isArray(input.edits) ? input.edits : [];
        return [
          fieldsDetail([
            displayField("文件", pick(input, FILE_FIELDS)),
            displayField("编辑数", edits.length || undefined),
          ]),
        ];
      },
    },
  },
  Write: {
    kind: "file_change",
    summaryFields: FILE_FIELDS,
    display: {
      action: "写入",
      icon: "file-pen",
      bucket: "file",
      unit: "个文件",
      extractObject: (input) => compactLine(pick(input, FILE_FIELDS), 1200),
      buildDetails: (input) => [
        fieldsDetail([displayField("文件", pick(input, FILE_FIELDS))]),
      ],
    },
  },
  NotebookEdit: {
    kind: "file_change",
    summaryFields: ["notebook_path", ...FILE_FIELDS],
    display: {
      action: "修改笔记本",
      icon: "file-pen",
      bucket: "file",
      unit: "个文件",
      extractObject: (input) =>
        compactLine(pick(input, ["notebook_path", ...FILE_FIELDS]), 1200),
      buildDetails: (input) => [
        fieldsDetail([
          displayField("笔记本", pick(input, ["notebook_path", ...FILE_FIELDS])),
        ]),
      ],
    },
  },
  Glob: {
    kind: "tool",
    summaryFields: ["pattern"],
    display: {
      action: "查找文件",
      icon: "search",
      bucket: "tool",
      unit: "个工具",
      extractObject: (input) => compactLine(pick(input, ["pattern"]), 1200),
      buildDetails: (input) => [
        fieldsDetail([
          displayField("pattern", pick(input, ["pattern"])),
          displayField("path", pick(input, ["path"])),
        ]),
      ],
    },
  },
  Grep: {
    kind: "tool",
    summaryFields: ["pattern"],
    display: {
      action: "搜索内容",
      icon: "search",
      bucket: "tool",
      unit: "个工具",
      extractObject: (input) => compactLine(pick(input, ["pattern"]), 1200),
      buildDetails: (input) => [
        fieldsDetail([
          displayField("pattern", pick(input, ["pattern"])),
          displayField("path", pick(input, ["path"])),
          displayField("glob", pick(input, ["glob"])),
        ]),
      ],
    },
  },
  WebSearch: {
    kind: "web_search",
    summaryFields: ["query"],
    display: {
      action: "网络搜索",
      icon: "search",
      bucket: "web_search",
      unit: "次搜索",
      extractObject: (input) => compactLine(pick(input, ["query"]), 1200),
      buildDetails: (input) => [
        fieldsDetail([displayField("查询", pick(input, ["query"]))]),
      ],
    },
  },
  WebFetch: {
    kind: "web_search",
    summaryFields: ["url"],
    display: {
      action: "抓取网页",
      icon: "globe",
      bucket: "web_search",
      unit: "次搜索",
      extractObject: (input) => compactLine(pick(input, ["url"]), 1200),
      buildDetails: (input) => [
        fieldsDetail([displayField("URL", pick(input, ["url"]))]),
      ],
    },
  },
  TodoWrite: {
    kind: "todo_list",
    summaryFields: [],
    display: {
      action: "更新待办",
      icon: "list-checks",
      bucket: "todo",
      unit: "次待办",
      extractObject: () => "",
      buildDetails: (input) => {
        const items = readTodoItems({ items: input.todos });
        return [listDetail(items)];
      },
    },
  },
  Task: {
    kind: "subagent",
    summaryFields: ["subagent_type", "description"],
    display: {
      action: "调用子代理",
      icon: "bot",
      bucket: "subagent",
      unit: "个子代理",
      extractObject: (input) =>
        compactLine(pick(input, ["subagent_type", "description"]), 200),
      buildDetails: (input) => [
        markdownDetail(pick(input, ["prompt", "description"]), "default"),
      ],
    },
  },
  ExitPlanMode: {
    kind: "plan",
    summaryFields: ["plan"],
    display: {
      action: "制定计划",
      icon: "list-ordered",
      bucket: "plan",
      unit: "项计划",
      extractObject: () => "",
      buildDetails: (input) => [markdownDetail(pick(input, ["plan"]))],
    },
  },
};

// Agent 是 Task 的别名（CLI 调度同一类事件）。
CLAUDE_TOOLS.Agent = CLAUDE_TOOLS.Task;

export const CLAUDE_TOOL_DEFAULT = {
  kind: "tool",
  summaryFields: [],
  display: {
    action: "调用工具",
    icon: "wrench",
    bucket: "tool",
    unit: "个工具",
    objectInLabel: true,
    extractObject: (_input, name) => name,
    buildDetails: (input, payload, name) => [
      fieldsDetail([displayField("工具", name)]),
      codeDetail("INPUT", input),
      codeDetail("OUTPUT", pick(payload, ["output"])),
    ],
  },
};

/** 按工具名查表，未登记则返回 CLAUDE_TOOL_DEFAULT。 */
export function getClaudeTool(name) {
  return CLAUDE_TOOLS[name] ?? CLAUDE_TOOL_DEFAULT;
}
