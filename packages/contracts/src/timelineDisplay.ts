/**
 * Timeline display 派生器：纯函数，从 `{kind, status, title, summary, payload}`
 * 算出 `AgentTimelineDisplay`。前端在渲染时调用，runner / Rust 端只负责存事实
 * （工具名、原始 input/output、文件路径、命令、todo 项等），不固化任何展示
 * 文本，这样改 display 规则可以即时影响历史数据。
 *
 * 设计原则：
 * - 只依赖 payload 中的「事实」字段，不读 display；display 字段在 contracts 里
 *   会被改成可选只用作运行时缓存。
 * - 工具事件优先走 CLAUDE_TOOL_DISPLAY 注册表（按 `payload.toolName` 命中），
 *   未登记的工具用 `CLAUDE_TOOL_DEFAULT`。
 * - 其它 kind（command/file_change/mcp/web_search/subagent/error/turn/message/
 *   reasoning/todo_list）走 kind 分支。
 * - 兜底返回 "处理 + 标题" 的简陋 display，绝不返回 null。
 */
import type {
  AgentTimelineDisplay,
  AgentTimelineDisplayDetail,
  AgentTimelineDisplayField,
  AgentTimelineDisplayListItem,
  AgentTimelineEventStatus,
  AgentTimelinePayload,
} from "./index";

export interface TimelineDisplayInput {
  kind: string;
  status: AgentTimelineEventStatus;
  title: string;
  summary: string | null;
  payload: AgentTimelinePayload;
}

export function deriveTimelineDisplay(input: TimelineDisplayInput): AgentTimelineDisplay {
  const kind = input.kind || "tool";
  const title = compactLine(input.title, 200) || kind;
  const summary = compactLine(input.summary ?? "", 1200);
  const payload = readRecord(input.payload);

  const declaredToolName = readFirstString(payload, ["toolName", "tool", "name"], 200);
  if (declaredToolName && isToolKind(kind)) {
    return cleanDisplay(
      buildClaudeToolDisplay(declaredToolName, readRecord(payload.input), payload),
    ) ?? fallbackDisplay(kind, title, summary);
  }

  return (
    cleanDisplay(buildByKind({ kind, status: input.status, title, summary, payload })) ??
    fallbackDisplay(kind, title, summary)
  );
}

// ---------- 工具注册表 ----------

interface ClaudeToolConfig {
  kind: string;
  action: string;
  icon: string;
  bucket: string;
  unit: string;
  objectInLabel?: boolean;
  extractObject: (input: Record<string, unknown>, name: string) => string;
  buildDetails: (
    input: Record<string, unknown>,
    payload: Record<string, unknown>,
    name: string,
  ) => Array<AgentTimelineDisplayDetail | null>;
}

const CLAUDE_TOOL_DISPLAY: Record<string, ClaudeToolConfig> = {
  Bash: {
    kind: "command",
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
  Read: {
    kind: "file_read",
    action: "读取",
    icon: "book-open",
    bucket: "file",
    unit: "个文件",
    extractObject: (input) => compactLine(pick(input, ["file_path", "path"]), 1200),
    buildDetails: (input) => [
      fieldsDetail([
        displayField("文件", pick(input, ["file_path", "path"])),
        displayField("offset", pick(input, ["offset"])),
        displayField("limit", pick(input, ["limit"])),
      ]),
    ],
  },
  Edit: {
    kind: "file_change",
    action: "修改",
    icon: "file-pen",
    bucket: "file",
    unit: "个文件",
    extractObject: (input) => compactLine(pick(input, ["file_path", "path"]), 1200),
    buildDetails: (input) => [
      fieldsDetail([displayField("文件", pick(input, ["file_path", "path"]))]),
    ],
  },
  MultiEdit: {
    kind: "file_change",
    action: "批量修改",
    icon: "file-pen",
    bucket: "file",
    unit: "个文件",
    extractObject: (input) => compactLine(pick(input, ["file_path", "path"]), 1200),
    buildDetails: (input) => {
      const edits = Array.isArray(input.edits) ? input.edits : [];
      return [
        fieldsDetail([
          displayField("文件", pick(input, ["file_path", "path"])),
          displayField("编辑数", edits.length || undefined),
        ]),
      ];
    },
  },
  Write: {
    kind: "file_change",
    action: "写入",
    icon: "file-pen",
    bucket: "file",
    unit: "个文件",
    extractObject: (input) => compactLine(pick(input, ["file_path", "path"]), 1200),
    buildDetails: (input) => [
      fieldsDetail([displayField("文件", pick(input, ["file_path", "path"]))]),
    ],
  },
  NotebookEdit: {
    kind: "file_change",
    action: "修改笔记本",
    icon: "file-pen",
    bucket: "file",
    unit: "个文件",
    extractObject: (input) =>
      compactLine(pick(input, ["notebook_path", "file_path", "path"]), 1200),
    buildDetails: (input) => [
      fieldsDetail([
        displayField("笔记本", pick(input, ["notebook_path", "file_path", "path"])),
      ]),
    ],
  },
  Glob: {
    kind: "tool",
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
  Grep: {
    kind: "tool",
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
  WebSearch: {
    kind: "web_search",
    action: "网络搜索",
    icon: "search",
    bucket: "web_search",
    unit: "次搜索",
    extractObject: (input) => compactLine(pick(input, ["query"]), 1200),
    buildDetails: (input) => [fieldsDetail([displayField("查询", pick(input, ["query"]))])],
  },
  WebFetch: {
    kind: "web_search",
    action: "抓取网页",
    icon: "globe",
    bucket: "web_search",
    unit: "次搜索",
    extractObject: (input) => compactLine(pick(input, ["url"]), 1200),
    buildDetails: (input) => [fieldsDetail([displayField("URL", pick(input, ["url"]))])],
  },
  TodoWrite: {
    kind: "todo_list",
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
  Task: {
    kind: "subagent",
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
  ExitPlanMode: {
    kind: "plan",
    action: "制定计划",
    icon: "list-ordered",
    bucket: "plan",
    unit: "项计划",
    extractObject: () => "",
    buildDetails: (input) => [markdownDetail(pick(input, ["plan"]))],
  },
};

CLAUDE_TOOL_DISPLAY.Agent = CLAUDE_TOOL_DISPLAY.Task;

const CLAUDE_TOOL_DEFAULT: ClaudeToolConfig = {
  kind: "tool",
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
};

/** kind 取值落在工具范畴时才查注册表 —— message/reasoning/turn/error 不该被工具规则覆写。 */
function isToolKind(kind: string): boolean {
  return (
    kind === "tool" ||
    kind === "command" ||
    kind === "file_change" ||
    kind === "file_read" ||
    kind === "todo_list" ||
    kind === "subagent" ||
    kind === "plan" ||
    kind === "web_search"
  );
}

function getToolConfig(name: string): ClaudeToolConfig {
  return CLAUDE_TOOL_DISPLAY[name] ?? CLAUDE_TOOL_DEFAULT;
}

function buildClaudeToolDisplay(
  name: string,
  input: Record<string, unknown>,
  payload: Record<string, unknown>,
): AgentTimelineDisplay {
  const config = getToolConfig(name);
  const object = config.extractObject(input, name) || "";
  const details = config
    .buildDetails(input, payload, name)
    .filter((detail): detail is AgentTimelineDisplayDetail => detail !== null);
  return {
    icon: config.icon,
    action: config.action,
    object,
    objectInLabel: config.objectInLabel === true ? true : undefined,
    preview: object || compactLine(pick(payload, ["output"]), 600),
    details: details.length ? details : undefined,
    group: {
      key: `tool:${name}`,
      bucket: config.bucket,
      unit: config.unit,
      count: 1,
    },
  };
}

// ---------- kind 分支 ----------

interface KindBuildInput {
  kind: string;
  status: AgentTimelineEventStatus;
  title: string;
  summary: string;
  payload: Record<string, unknown>;
}

function buildByKind({ kind, title, summary, payload }: KindBuildInput): AgentTimelineDisplay {
  switch (kind) {
    case "message": {
      const role = readFirstString(payload, ["role"], 80);
      return {
        icon: "message-square",
        label: role === "assistant" ? "Assistant" : title,
        preview: summary || readFirstString(payload, ["content"], 600),
        defaultExpanded: role === "assistant" ? true : undefined,
      };
    }
    case "reasoning":
      return {
        action: "思考",
        preview: summary || readFirstString(payload, ["text", "summary"], 600),
        details: [markdownDetail(summary || pick(payload, ["text", "summary"]), "muted")]
          .filter((d): d is AgentTimelineDisplayDetail => d !== null),
      };
    case "todo_list": {
      const items = readTodoItems(payload);
      return {
        icon: "list-checks",
        action: "更新待办",
        preview: summary || todoPreview(items),
        details: [lineDetail(summary), listDetail(items)]
          .filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: "kind:todo_list", bucket: "todo", unit: "次待办", count: 1 },
      };
    }
    case "command": {
      const nestedInput = readRecord(payload.input);
      // 不回落到 title：title 通常是工具名 "Bash"，灌进 object 会变成"已运行 Bash"。
      const command =
        readFirstString(payload, ["command", "cmd", "shellCommand", "script", "argv"], 1200) ||
        readFirstString(nestedInput, ["command", "cmd", "shellCommand", "script", "argv"], 1200);
      const output = readFirstString(
        payload,
        ["aggregatedOutput", "combinedOutput", "outputText", "stdout"],
        6000,
      );
      const stderr = readFirstString(
        payload,
        ["stderr", "errorOutput", "message", "error"],
        6000,
      );
      return {
        icon: "terminal",
        action: "运行",
        object: command,
        preview: summary || command || output || stderr,
        details: [
          lineDetail(summary),
          fieldsDetail([
            displayField("cwd", pick(payload, ["cwd", "workdir", "workingDirectory"])),
            displayField("exit", pick(payload, ["exitCode", "code", "statusCode"])),
            displayField("duration", formatDuration(payload)),
          ]),
          codeDetail("COMMAND", command, "shell"),
          codeDetail(stderr ? "ERROR / OUTPUT" : "OUTPUT", output || stderr),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: "kind:command", bucket: "command", unit: "条命令", count: 1 },
      };
    }
    case "file_change": {
      const changes = readFileChanges(payload);
      const count = changes.length || 1;
      return {
        icon: "file-pen",
        action: "修改",
        object: fileChangeObject(changes, payload) || usefulObject(title, ["file change", "file changes"]),
        preview: summary || fileChangePreview(changes, payload),
        details: [
          lineDetail(summary),
          listDetail(changes.map((change) => `${change.kind} ${change.path}`)),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: "kind:file_change", bucket: "file", unit: "个文件", count },
      };
    }
    case "mcp": {
      const target = [
        readFirstString(payload, ["server", "serverName", "mcpServer"], 200),
        readFirstString(payload, ["tool", "toolName", "name"], 200),
      ]
        .filter(Boolean)
        .join("/");
      return {
        icon: "plug",
        action: "调用 MCP",
        object: target || usefulObject(title, ["mcp", "mcp tool"]),
        objectInLabel: true,
        preview: summary || target,
        details: [
          fieldsDetail([
            displayField("服务", pick(payload, ["server", "serverName", "mcpServer"])),
            displayField("工具", pick(payload, ["tool", "toolName", "name"])),
          ]),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: {
          key: `mcp:${readFirstString(payload, ["server", "serverName", "mcpServer"], 120) || "default"}`,
          bucket: "mcp",
          unit: "次 MCP",
          count: 1,
        },
      };
    }
    case "web_search": {
      const query = readFirstString(payload, ["query", "searchQuery", "q", "url"], 1200);
      return {
        icon: "search",
        action: "网络搜索",
        object: query || usefulObject(title, ["web search", "search"]),
        preview: summary || query,
        details: [fieldsDetail([displayField("查询", query)])]
          .filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: "kind:web_search", bucket: "web_search", unit: "次搜索", count: 1 },
      };
    }
    case "subagent": {
      const name =
        readFirstString(
          payload,
          ["agentType", "subagentType", "agentName", "taskType", "name", "type"],
          200,
        ) || usefulObject(title, ["task", "agent"]);
      const task = readFirstString(
        payload,
        ["taskDescription", "description", "prompt", "task"],
        1200,
      );
      const result = readFirstString(payload, ["result", "output", "summary"], 1200);
      return {
        icon: "bot",
        action: "调用子代理",
        object: name,
        preview: summary || [name, task].filter(Boolean).join(": "),
        details: [
          markdownDetail(task, "default"),
          markdownDetail(result, "default"),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: "kind:subagent", bucket: "subagent", unit: "个子代理", count: 1 },
      };
    }
    case "plan": {
      const plan = readFirstString(payload, ["plan", "content", "text"], 6000);
      return {
        icon: "list-ordered",
        action: "制定计划",
        object: title,
        preview: summary || plan,
        details: [markdownDetail(plan || summary)].filter(
          (d): d is AgentTimelineDisplayDetail => d !== null,
        ),
        group: { key: "kind:plan", bucket: "plan", unit: "项计划", count: 1 },
      };
    }
    case "error": {
      const message =
        summary ||
        readFirstString(payload, ["message", "error", "reason", "details", "stderr"], 1200);
      return {
        icon: "alert-triangle",
        label: title || "错误",
        preview: message,
        details: [
          lineDetail(message, "muted"),
          fieldsDetail([
            displayField("code", pick(payload, ["code", "exitCode", "statusCode"])),
            displayField("path", pick(payload, ["file", "filePath", "path"])),
            displayField("command", pick(payload, ["command", "cmd", "shellCommand"])),
          ]),
          codeDetail("STACK", pick(payload, ["stack", "trace", "backtrace"])),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: "kind:error", bucket: "error", unit: "个错误", count: 1 },
      };
    }
    case "turn":
      return {
        label: title,
        preview:
          summary || readFirstString(payload, ["status", "eventType", "subtype", "state"], 600),
        details: [
          lineDetail(summary),
          fieldsDetail([
            displayField("backend", pick(payload, ["backend"])),
            displayField(
              "event",
              pick(payload, ["eventType", "subtype", "status", "state"]),
            ),
            displayField("session", pick(payload, ["sessionId"])),
          ]),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
      };
    case "tool":
    default: {
      const tool =
        readFirstString(payload, ["toolName", "name", "tool", "function", "hookName"], 200) ||
        usefulObject(title, ["tool"]);
      const input = pickValue(payload, [
        "input",
        "arguments",
        "args",
        "parameters",
        "params",
        "request",
      ]);
      const output = pickValue(payload, ["result", "response", "output", "text", "content"]);
      return {
        icon: "wrench",
        action: kind === "tool" ? "调用工具" : "处理",
        object: tool || title,
        objectInLabel: true,
        preview:
          summary || tool || readFirstString(payload, ["query", "path", "command"], 600),
        details: [
          fieldsDetail([
            displayField("工具", tool),
            displayField("服务", pick(payload, ["server", "serverName", "mcpServer"])),
          ]),
          codeDetail("INPUT", input),
          codeDetail("OUTPUT", output),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: `tool:${tool || title || kind}`, bucket: "tool", unit: "个工具", count: 1 },
      };
    }
  }
}

function fallbackDisplay(kind: string, title: string, summary: string): AgentTimelineDisplay {
  return {
    action: "处理",
    object: title || kind || "事件",
    objectInLabel: true,
    preview: summary || title || kind || "",
    group: {
      key: `kind:${kind || "event"}`,
      bucket: "other",
      unit: "项",
      count: 1,
    },
  };
}

// ---------- 通用 helper ----------

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function readRecord(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {};
}

function pick(record: Record<string, unknown>, keys: string[]): unknown {
  for (const key of keys) {
    const value = record[key];
    if (value !== undefined && value !== null && value !== "") return value;
  }
  return undefined;
}

function pickValue(record: Record<string, unknown>, keys: string[]): unknown {
  for (const key of keys) {
    if (record[key] !== undefined) return record[key];
  }
  return undefined;
}

function stringOrNull(value: unknown): string | null {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return null;
}

function shortText(value: unknown, max: number): string {
  const text = stringOrNull(value);
  if (!text) return "";
  return text.length > max ? `${text.slice(0, max)}...` : text;
}

function stringifyInline(value: unknown): string {
  if (typeof value === "string") return value.trim();
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) {
    return value.map((item) => stringifyInline(item)).filter(Boolean).join(" ").trim();
  }
  if (isRecord(value)) {
    return readFirstString(value, [
      "text",
      "title",
      "summary",
      "content",
      "message",
      "name",
      "path",
      "filePath",
      "query",
      "command",
    ], 600);
  }
  return "";
}

function compactLine(value: unknown, max: number): string {
  const text = stringifyInline(value).replace(/\s+/g, " ").trim();
  return text ? shortText(text, max) : "";
}

function readFirstString(
  payload: Record<string, unknown>,
  keys: string[],
  max: number,
): string {
  for (const key of keys) {
    const text = compactLine(payload[key], max);
    if (text) return text;
  }
  return "";
}

function displayField(label: string, value: unknown): AgentTimelineDisplayField | null {
  const text = compactLine(value, 1200);
  return label && text ? { label, value: text } : null;
}

function lineDetail(
  text: unknown,
  tone: "default" | "muted" = "muted",
): AgentTimelineDisplayDetail | null {
  const content = compactLine(text, 1200);
  return content ? { type: "line", text: content, tone } : null;
}

function fieldsDetail(
  fields: Array<AgentTimelineDisplayField | null>,
): AgentTimelineDisplayDetail | null {
  const items = fields.filter((field): field is AgentTimelineDisplayField => field !== null);
  return items.length ? { type: "fields", fields: items } : null;
}

function codeDetail(
  label: string,
  content: unknown,
  language: string = "",
): AgentTimelineDisplayDetail | null {
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

function markdownDetail(
  content: unknown,
  tone: "default" | "muted" = "default",
  singleLine: boolean = false,
): AgentTimelineDisplayDetail | null {
  const text = stringOrNull(content);
  if (!text || !text.trim()) return null;
  return {
    type: "markdown",
    content: shortText(text.trim(), 6000),
    tone,
    singleLine,
  };
}

function listDetail(
  items: unknown,
  ordered: boolean = false,
): AgentTimelineDisplayDetail | null {
  const normalized = (Array.isArray(items) ? items : [])
    .map((item): AgentTimelineDisplayListItem | null => {
      if (typeof item === "string") return { text: compactLine(item, 1200) };
      if (!isRecord(item)) return null;
      const text = readFirstString(item, ["text", "content", "title", "summary"], 1200);
      if (!text) return null;
      const status = String(item.status ?? "").toLowerCase();
      const completed = item.completed === true || item.done === true || status === "completed";
      const tone: AgentTimelineDisplayListItem["tone"] = completed
        ? "success"
        : status === "failed" || status === "error"
          ? "error"
          : "default";
      return { text, tone };
    })
    .filter((item): item is AgentTimelineDisplayListItem => item !== null && Boolean(item.text));
  return normalized.length ? { type: "list", items: normalized, ordered } : null;
}

function usefulObject(title: string, generic: string[]): string {
  const text = compactLine(title, 300);
  if (!text) return "";
  const normalized = text.toLowerCase();
  return generic.map((g) => g.toLowerCase()).includes(normalized) ? "" : text;
}

function formatDuration(payload: Record<string, unknown>): string {
  const raw = pickValue(payload, ["durationMs", "elapsedMs", "duration"]);
  if (typeof raw === "number") {
    return raw >= 1000 ? `${(raw / 1000).toFixed(1)}s` : `${raw}ms`;
  }
  return compactLine(raw, 80);
}

interface ParsedTodoItem {
  text: string;
  completed: boolean;
  status?: string;
}

function readTodoItems(payload: Record<string, unknown>): ParsedTodoItem[] {
  const input = readRecord(payload.input);
  const raw =
    (Array.isArray(payload.items) && payload.items) ||
    (Array.isArray(payload.todos) && payload.todos) ||
    (Array.isArray(input.items) && input.items) ||
    (Array.isArray(input.todos) && input.todos) ||
    [];
  return raw
    .map((item: unknown): ParsedTodoItem | null => {
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
    .filter((item): item is ParsedTodoItem => item !== null);
}

function todoPreview(items: ParsedTodoItem[]): string {
  if (!items.length) return "";
  const done = items.filter((item) => item.completed).length;
  const next = items.find((item) => !item.completed)?.text ?? "";
  return `${done}/${items.length} 已完成${next ? ` · ${next}` : ""}`;
}

interface FileChange {
  kind: string;
  path: string;
}

function readFileChanges(payload: Record<string, unknown>): FileChange[] {
  const input = readRecord(payload.input);
  const args = readRecord(payload.args);
  const parameters = readRecord(payload.parameters);
  const raw =
    (Array.isArray(payload.changes) && payload.changes) ||
    (Array.isArray(input.changes) && input.changes) ||
    (Array.isArray(args.changes) && args.changes) ||
    (Array.isArray(parameters.changes) && parameters.changes) ||
    [];
  return raw
    .map((change: unknown): FileChange | null => {
      if (!isRecord(change)) return null;
      const path = readFirstString(
        change,
        ["path", "filePath", "relativePath", "targetPath", "name"],
        600,
      );
      if (!path) return null;
      return {
        kind: readFirstString(change, ["kind", "operation", "type", "status"], 80) || "update",
        path,
      };
    })
    .filter((change): change is FileChange => change !== null);
}

function fileChangeObject(changes: FileChange[], payload: Record<string, unknown>): string {
  if (changes.length) return changes[0].path;
  return readFirstString(
    payload,
    ["path", "filePath", "relativePath", "targetPath", "name"],
    600,
  );
}

function fileChangePreview(changes: FileChange[], payload: Record<string, unknown>): string {
  if (changes.length) {
    const first = changes[0];
    const suffix = changes.length > 1 ? ` 等 ${changes.length} 个文件` : "";
    return `${first.kind} ${first.path}${suffix}`;
  }
  const path = fileChangeObject(changes, payload);
  if (!path) return "";
  const kind = readFirstString(payload, ["kind", "operation", "type", "status"], 80) || "update";
  return `${kind} ${path}`;
}

function cleanDisplay(display: AgentTimelineDisplay | null): AgentTimelineDisplay | null {
  if (!display) return null;
  const entries = Object.entries(display).filter(([, value]) => {
    if (value === undefined || value === null) return false;
    if (typeof value === "string" && value === "") return false;
    if (Array.isArray(value) && value.length === 0) return false;
    return true;
  });
  return entries.length ? (Object.fromEntries(entries) as AgentTimelineDisplay) : null;
}
