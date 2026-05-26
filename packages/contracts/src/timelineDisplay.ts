/**
 * Timeline display 派生器：纯函数，从 `{kind, status, title, summary, payload}`
 * 算出 `AgentTimelineDisplay`。前端在渲染时调用，runner / Rust 端只负责存事实
 * （工具名、原始 input/output、文件路径、命令、todo 项等），不固化任何展示
 * 文本，这样改 display 规则可以即时影响历史数据。
 *
 * 设计原则：
 * - 工具事件（payload.toolName 命中工具范畴 kind）走 `./claudeTools.mjs` 里的
 *   CLAUDE_TOOLS 表 —— 同一张表里既有 lilia 的 kind 分类、runner 用的 summary
 *   字段，也有渲染规则。改一处两边同步。
 * - 其它 kind（message/reasoning/command/file_change/mcp/web_search/subagent/
 *   plan/error/turn/todo_list/tool default）走本文件 buildByKind 分支。
 * - 兜底返回 "处理 + 标题" 的简陋 display，绝不返回 null。
 */
import {
  getClaudeTool,
  compactLine,
  pick,
  readFirstString,
  readRecord,
  readTodoItems,
  displayField,
  fieldsDetail,
  codeDetail,
  markdownDetail,
  listDetail,
  type ParsedTodoItem,
} from "./claudeTools.mjs";
import type {
  AgentTimelineDisplay,
  AgentTimelineDisplayDetail,
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

/** kind 取值落在工具范畴时才查 CLAUDE_TOOLS —— message/reasoning/turn/error 不该被工具规则覆写。 */
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

function buildClaudeToolDisplay(
  name: string,
  input: Record<string, unknown>,
  payload: Record<string, unknown>,
): AgentTimelineDisplay {
  const config = getClaudeTool(name).display;
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

// ---------- TS-only helper（不需要跨到 runner） ----------

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function pickValue(record: Record<string, unknown>, keys: string[]): unknown {
  for (const key of keys) {
    if (record[key] !== undefined) return record[key];
  }
  return undefined;
}

function lineDetail(
  text: unknown,
  tone: "default" | "muted" = "muted",
): AgentTimelineDisplayDetail | null {
  const content = compactLine(text, 1200);
  return content ? { type: "line", text: content, tone } : null;
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

// AgentTimelineDisplayListItem 仅在 lineDetail/listDetail 内部被 .mjs 闭包构造，
// 这里 re-export 保证 .d.mts 类型链路完整。
export type { AgentTimelineDisplayListItem };
