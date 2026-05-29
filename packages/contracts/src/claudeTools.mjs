// Claude SDK → lilia 工具协议适配层。runner 在收到 @anthropic-ai/claude-agent-sdk
// 的 tool_use 块时，按 `CLAUDE_TO_LILIA` 表把 (toolName, input, payload) 翻译成
// lilia 协议事件 `{kind, subkind?, payload}`，再 emit 进 NDJSON。
//
// 设计原则：
//  - 本文件只 runner 用，渲染器不读 —— 渲染器只认 lilia 协议（liliaTools.mjs）。
//  - 未登记的工具名走 `CLAUDE_DEFAULT_NORMALIZE`，落 lilia 的 tool 兜底 kind。
//  - 新增 Claude 工具：在 `CLAUDE_TO_LILIA` 加一条；不要在 liliaTools.mjs 里硬编码工具名。
//
// 这是 .mjs 而非 .ts 是因为 runner 由 Tauri 直接 `node agent-runner.mjs` 拉起，
// 不经过任何构建步骤；TS 端通过同目录 claudeTools.d.mts 拿到类型。

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function readRecord(value) {
  return isRecord(value) ? value : {};
}

function pickString(record, keys) {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  return "";
}

function pickNumber(record, keys) {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "number" && Number.isFinite(value)) return value;
  }
  return undefined;
}

function shortText(value, max) {
  if (typeof value !== "string") return "";
  return value.length > max ? `${value.slice(0, max)}...` : value;
}

const FILE_PATH_KEYS = ["file_path", "path"];

function compactLine(value, max) {
  if (typeof value !== "string") return "";
  const text = value.replace(/\s+/g, " ").trim();
  return text.length > max ? `${text.slice(0, max)}...` : text;
}

function readAskUserQuestions(input) {
  return (Array.isArray(input?.questions) ? input.questions : [])
    .filter(isRecord);
}

function askUserQuestionTitle(question, index) {
  const header = compactLine(question?.header, 40);
  const text = compactLine(question?.question, 240);
  if (header && text) return `${header} · ${text}`;
  return text || header || `问题 ${index + 1}`;
}

function askUserPreview(questions) {
  if (!questions.length) return "用户提问";
  const title = askUserQuestionTitle(questions[0], 0);
  if (questions.length <= 1) return title;
  return `${title} 等 ${questions.length} 个问题`;
}

function normalizeAskUserQuestionTool(input) {
  const questions = readAskUserQuestions(input);
  return {
    kind: "ask_user",
    payload: { questions },
    summary: shortText(askUserPreview(questions), 200),
  };
}

function readPlanAllowedPrompts(input) {
  return (Array.isArray(input?.allowedPrompts) ? input.allowedPrompts : [])
    .filter(isRecord)
    .map((item) => ({
      tool: compactLine(item.tool, 80) || "tool",
      prompt: compactLine(item.prompt, 400),
    }))
    .filter((item) => item.prompt);
}

/**
 * Claude 工具名 → lilia 协议规范化器。
 *
 * 每个条目 `(input, payload) => { kind, subkind?, payload, summary? }`：
 *  - kind/subkind：lilia 协议分类
 *  - payload：lilia 协议字段（已挑过的键，不要塞整个 input）
 *  - summary：runner 写入 timeline event 的 summary 字段（默认空串）
 *
 * payload 传进来的是 SDK 给的原始 input + 周边 metadata 合并后的对象，
 * 适配器只挑感兴趣的字段往 lilia payload 上拷。
 */
export const CLAUDE_TO_LILIA = {
  AskUserQuestion: normalizeAskUserQuestionTool,
  ask_user_question: normalizeAskUserQuestionTool,
  mcp__lilia__ask_user_question: normalizeAskUserQuestionTool,
  Bash: (input) => {
    const command = pickString(input, ["command"]);
    return {
      kind: "command",
      payload: {
        command,
        description: pickString(input, ["description"]) || undefined,
      },
      summary: shortText(command || pickString(input, ["description"]), 200),
    };
  },
  Read: (input) => {
    const path = pickString(input, FILE_PATH_KEYS);
    return {
      kind: "file_read",
      payload: {
        path,
        offset: pickNumber(input, ["offset"]),
        limit: pickNumber(input, ["limit"]),
      },
      summary: shortText(path, 200),
    };
  },
  Edit: (input) => {
    const path = pickString(input, FILE_PATH_KEYS);
    return {
      kind: "file_change",
      subkind: "edit",
      payload: { path },
      summary: shortText(path, 200),
    };
  },
  MultiEdit: (input) => {
    const path = pickString(input, FILE_PATH_KEYS);
    const edits = Array.isArray(input?.edits) ? input.edits : [];
    return {
      kind: "file_change",
      subkind: "multi_edit",
      payload: {
        path,
        editCount: edits.length || undefined,
      },
      summary: shortText(path, 200),
    };
  },
  Write: (input) => {
    const path = pickString(input, FILE_PATH_KEYS);
    return {
      kind: "file_change",
      subkind: "write",
      payload: { path },
      summary: shortText(path, 200),
    };
  },
  NotebookEdit: (input) => {
    const path = pickString(input, ["notebook_path", ...FILE_PATH_KEYS]);
    return {
      kind: "file_change",
      subkind: "notebook",
      payload: { path },
      summary: shortText(path, 200),
    };
  },
  Glob: (input) => {
    const query = pickString(input, ["pattern"]);
    return {
      kind: "search",
      subkind: "glob",
      payload: {
        query,
        path: pickString(input, ["path"]) || undefined,
      },
      summary: shortText(query, 200),
    };
  },
  Grep: (input) => {
    const query = pickString(input, ["pattern"]);
    return {
      kind: "search",
      subkind: "grep",
      payload: {
        query,
        path: pickString(input, ["path"]) || undefined,
        glob: pickString(input, ["glob"]) || undefined,
      },
      summary: shortText(query, 200),
    };
  },
  WebSearch: (input) => {
    const query = pickString(input, ["query"]);
    return {
      kind: "search",
      subkind: "web",
      payload: { query },
      summary: shortText(query, 200),
    };
  },
  WebFetch: (input) => {
    const url = pickString(input, ["url"]);
    return {
      kind: "web_fetch",
      payload: { url },
      summary: shortText(url, 200),
    };
  },
  TodoWrite: (input) => {
    const todos = Array.isArray(input?.todos) ? input.todos : [];
    return {
      kind: "todo_list",
      payload: { items: todos },
      summary: "",
    };
  },
  Task: (input, ctx) => {
    const agentType = pickString(input, ["subagent_type"])
      || pickString(ctx ?? {}, ["subagent_type", "subagentType"]);
    const description = pickString(input, ["description"])
      || pickString(ctx ?? {}, ["task_description", "taskDescription", "description"]);
    const prompt = pickString(input, ["prompt"]);
    return {
      kind: "subagent",
      payload: {
        agentType,
        description: description || undefined,
        prompt: prompt || undefined,
      },
      summary: shortText(agentType || description, 200),
    };
  },
  ExitPlanMode: (input) => {
    const plan = pickString(input, ["plan", "content", "text", "markdown"]);
    return {
      kind: "plan",
      payload: {
        source: "ExitPlanMode",
        plan,
        allowedPrompts: readPlanAllowedPrompts(input),
      },
      summary: shortText(plan.replace(/\s+/g, " ").trim(), 200),
    };
  },
};

// Agent 是 Task 的别名（CLI 调度同一类事件）。
CLAUDE_TO_LILIA.Agent = CLAUDE_TO_LILIA.Task;

/**
 * 未登记工具的兜底：归到 lilia 的 tool kind，把原始 input 透传。
 * 这样下游 deriveLiliaToolDisplay 能给出 "已调用工具 <name>" 的渲染。
 */
function defaultNormalize(name, input) {
  return {
    kind: "tool",
    payload: {
      toolName: name,
      input,
    },
    summary: "",
  };
}

/**
 * 适配 Claude 工具调用为 lilia 协议事件。
 *
 * @param name    Claude 工具名（block.name）
 * @param input   block.input
 * @param ctx     可选：周边元数据，用于补 subagent_type 等
 * @returns       { kind, subkind?, payload, summary }
 */
export function normalizeClaudeTool(name, input, ctx) {
  const safeName = typeof name === "string" && name ? name : "tool";
  const safeInput = readRecord(input);
  const normalizer = CLAUDE_TO_LILIA[safeName];
  if (normalizer) {
    const result = normalizer(safeInput, ctx ?? null);
    return {
      kind: result.kind,
      subkind: result.subkind ?? null,
      payload: result.payload ?? {},
      summary: result.summary ?? "",
    };
  }
  return defaultNormalize(safeName, safeInput);
}
