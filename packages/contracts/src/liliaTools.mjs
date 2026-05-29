// Lilia 工具协议 —— backend-agnostic 的事件分类 + 渲染规则。
//
// 协议形状：每条 timeline 事件按 `{kind, subkind?, payload}` 描述事实。
//
// | kind          | 必须 payload   | 可选 payload                                  | 常见 subkind        |
// |---------------|----------------|-----------------------------------------------|---------------------|
// | command       | command        | cwd / exit / output / stderr / duration       | —                   |
// | file_read     | path           | offset / limit                                | —                   |
// | file_change   | path           | editCount                                     | edit / multi_edit / write / notebook |
// | search        | query          | path / glob                                   | glob / grep / web   |
// | web_fetch     | url            | —                                             | —                   |
// | subagent      | agentType      | description / prompt / result                 | —                   |
// | plan          | plan           | approved / allowedPrompts / revisionRequest  | —                   |
// | todo_list     | items[]        | —                                             | —                   |
// | ask_user      | questions[]    | output / result / cancelled                   | —                   |
// | tool          | toolName       | input / output                                | —（兜底）           |
//
// 派生 display 走 `deriveLiliaToolDisplay({kind, subkind, payload, ...})` —— 渲染时
// 现算的视图缓存，绝不持久化到 DB。改派生规则可即时影响历史事件。
//
// 这是 .mjs 而非 .ts：runner 由 Tauri 直接 `node agent-runner.mjs` 拉起，
// 不经过任何构建步骤；TS 端通过同目录 liliaTools.d.mts 拿到类型。

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

export function readFirstText(payload, keys, max) {
  for (const key of keys) {
    const text = shortText(stringOrNull(payload[key])?.trim() ?? "", max);
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
  const raw =
    (Array.isArray(payload.items) && payload.items) ||
    (Array.isArray(payload.todos) && payload.todos) ||
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

function readAskUserQuestions(payload) {
  const nestedInput = readRecord(payload.input);
  const raw =
    (Array.isArray(payload.questions) && payload.questions) ||
    (Array.isArray(nestedInput.questions) && nestedInput.questions) ||
    [];
  return raw
    .map((question) => isRecord(question) ? question : null)
    .filter((question) => question !== null);
}

function askUserQuestionTitle(question, index) {
  const header = compactLine(pick(question, ["header"]), 40);
  const text = compactLine(pick(question, ["question", "title", "text"]), 1200);
  if (header && text) return `${header} · ${text}`;
  return text || header || `问题 ${index + 1}`;
}

function askUserPreviewFromQuestions(questions) {
  if (!questions.length) return "用户提问";
  const title = askUserQuestionTitle(questions[0], 0);
  return questions.length > 1 ? `${title} 等 ${questions.length} 个问题` : title;
}

function parseRecordJson(value) {
  if (isRecord(value)) return value;
  if (typeof value !== "string" || !value.trim()) return null;
  try {
    const parsed = JSON.parse(value);
    return isRecord(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function readAskUserResult(payload) {
  for (const key of ["result", "structuredContent", "output"]) {
    const result = parseRecordJson(payload[key]);
    if (result) return result;
  }
  return {};
}

function findAskUserQuestion(questions, key, fallbackIndex) {
  const matchedIndex = questions.findIndex((question, index) => {
    const id = compactLine(pick(question, ["id"]), 120);
    const text = compactLine(pick(question, ["question", "title", "text"]), 1200);
    return key === id || key === text || key === askUserQuestionTitle(question, index);
  });
  const index = matchedIndex >= 0 ? matchedIndex : fallbackIndex;
  return questions[index] ? { question: questions[index], index } : null;
}

function formatAskUserAnswerValue(answer) {
  const answerRecord = readRecord(answer);
  if (answerRecord.skipped === true) return "已跳过";
  const value = answerRecord.value !== undefined ? answerRecord.value : answer;
  const valueText = Array.isArray(value)
    ? value.map((item) => compactLine(item, 1200)).filter(Boolean).join("、")
    : compactLine(value, 1200);
  const noteText = compactLine(pick(answerRecord, ["notes"]), 1200);
  if (valueText === "other" && noteText) return noteText;
  return noteText && valueText && !valueText.includes(noteText)
    ? `${valueText}（备注：${noteText}）`
    : valueText || noteText;
}

function askUserAnswerItems(payload) {
  const result = readAskUserResult(payload);
  const answers = readRecord(result.answers);
  const annotations = readRecord(result.annotations);
  const questions = readAskUserQuestions(payload);

  return Object.entries(answers)
    .map(([key, answer], index) => {
      const questionEntry = findAskUserQuestion(questions, key, index);
      const question = questionEntry?.question ?? {};
      const title = questionEntry
        ? askUserQuestionTitle(question, questionEntry.index)
        : compactLine(key, 1200) || `问题 ${index + 1}`;
      const annotation = readRecord(annotations[key]);
      const valueText = formatAskUserAnswerValue(answer);
      const noteText = compactLine(pick(annotation, ["notes"]), 1200);
      const answerText = noteText && valueText && !valueText.includes(noteText)
        ? `${valueText}（备注：${noteText}）`
        : valueText || noteText;
      return answerText ? `${title}：${answerText}` : title;
    })
    .filter(Boolean);
}

function askUserCancelled(payload) {
  const result = readAskUserResult(payload);
  return payload.cancelled === true || result.cancelled === true;
}

const LILIA_TOOL_REGISTRY = {
  command: {
    default: {
      action: "运行",
      icon: "terminal",
      bucket: "command",
      unit: "条命令",
      build(payload) {
        const command = compactLine(pick(payload, ["command"]), 1200);
        const output = readFirstText(payload, ["output", "stdout"], 6000);
        const stderr = readFirstText(payload, ["stderr"], 6000);
        return {
          object: command,
          details: [
            fieldsDetail([
              displayField("cwd", pick(payload, ["cwd"])),
              displayField("exit", pick(payload, ["exit", "exitCode"])),
              displayField("duration", pick(payload, ["duration"])),
            ]),
            codeDetail("COMMAND", command, "shell"),
            codeDetail(stderr ? "ERROR / OUTPUT" : "OUTPUT", output || stderr),
          ],
        };
      },
    },
  },
  file_read: {
    default: {
      action: "读取",
      icon: "book-open",
      bucket: "file",
      unit: "个文件",
      build(payload) {
        const path = compactLine(pick(payload, ["path"]), 1200);
        return {
          object: path,
          details: [
            fieldsDetail([
              displayField("文件", path),
              displayField("offset", pick(payload, ["offset"])),
              displayField("limit", pick(payload, ["limit"])),
            ]),
          ],
        };
      },
    },
  },
  file_change: {
    default: {
      action: "修改",
      icon: "file-pen",
      bucket: "file",
      unit: "个文件",
      build: buildFileChangeDisplay,
    },
    subkinds: {
      edit: {
        action: "修改",
        icon: "file-pen",
        bucket: "file",
        unit: "个文件",
        build: buildFileChangeDisplay,
      },
      multi_edit: {
        action: "批量修改",
        icon: "file-pen",
        bucket: "file",
        unit: "个文件",
        build(payload) {
          const path = compactLine(pick(payload, ["path"]), 1200);
          const editCount = pick(payload, ["editCount"]);
          return {
            object: path,
            details: [
              fieldsDetail([
                displayField("文件", path),
                displayField("编辑数", editCount),
              ]),
            ],
          };
        },
      },
      write: {
        action: "写入",
        icon: "file-pen",
        bucket: "file",
        unit: "个文件",
        build: buildFileChangeDisplay,
      },
      notebook: {
        action: "修改笔记本",
        icon: "file-pen",
        bucket: "file",
        unit: "个文件",
        build(payload) {
          const path = compactLine(pick(payload, ["path"]), 1200);
          return {
            object: path,
            details: [fieldsDetail([displayField("笔记本", path)])],
          };
        },
      },
    },
  },
  search: {
    default: {
      action: "搜索",
      icon: "search",
      bucket: "search",
      unit: "次搜索",
      build: buildSearchDisplay,
    },
    subkinds: {
      glob: {
        action: "查找文件",
        icon: "search",
        bucket: "search",
        unit: "次搜索",
        build: buildSearchDisplay,
      },
      grep: {
        action: "搜索内容",
        icon: "search",
        bucket: "search",
        unit: "次搜索",
        build: buildSearchDisplay,
      },
      web: {
        action: "网络搜索",
        icon: "search",
        bucket: "search",
        unit: "次搜索",
        build(payload) {
          const query = compactLine(pick(payload, ["query"]), 1200);
          return {
            object: query,
            details: [fieldsDetail([displayField("查询", query)])],
          };
        },
      },
    },
  },
  web_fetch: {
    default: {
      action: "抓取网页",
      icon: "globe",
      bucket: "search",
      unit: "次搜索",
      build(payload) {
        const url = compactLine(pick(payload, ["url"]), 1200);
        return {
          object: url,
          details: [fieldsDetail([displayField("URL", url)])],
        };
      },
    },
  },
  subagent: {
    default: {
      action: "调用子代理",
      icon: "bot",
      bucket: "subagent",
      unit: "个子代理",
      build(payload) {
        const agentType = compactLine(pick(payload, ["agentType"]), 200);
        const description = compactLine(pick(payload, ["description"]), 1200);
        const prompt = compactLine(pick(payload, ["prompt"]), 6000);
        const result = compactLine(pick(payload, ["result"]), 6000);
        return {
          object: agentType,
          details: [
            markdownDetail(description, "default"),
            markdownDetail(prompt, "default"),
            markdownDetail(result, "default"),
          ],
        };
      },
    },
  },
  plan: {
    default: {
      action: "制定计划",
      icon: "list-ordered",
      bucket: "plan",
      unit: "项计划",
      build(payload) {
        const plan = readFirstText(payload, ["plan"], 6000);
        const revisionRequest = readFirstText(payload, ["revisionRequest"], 6000);
        const preview = revisionRequest
          ? `修改要求：${compactLine(revisionRequest, 600)}`
          : compactLine(plan, 600);
        const approved = payload.approved;
        const allowedPrompts = (Array.isArray(payload.allowedPrompts) ? payload.allowedPrompts : [])
          .filter(isRecord)
          .map((item) => {
            const tool = compactLine(pick(item, ["tool"]), 80);
            const prompt = compactLine(pick(item, ["prompt"]), 400);
            return [tool, prompt].filter(Boolean).join("：");
          })
          .filter(Boolean);
        const label = revisionRequest
          ? "要求修改计划"
          : approved === null
          ? "等待确认计划"
          : approved === true
            ? "已确认计划"
            : approved === false
              ? "已取消计划"
              : undefined;
        return {
          object: "",
          label,
          preview,
          defaultExpanded: approved === null ? true : undefined,
          details: [
            markdownDetail(plan),
            markdownDetail(revisionRequest, "muted"),
            allowedPrompts.length ? listDetail(allowedPrompts, true) : null,
          ],
        };
      },
    },
  },
  todo_list: {
    default: {
      action: "更新待办",
      icon: "list-checks",
      bucket: "todo",
      unit: "次待办",
      build(payload) {
        const items = readTodoItems(payload);
        return {
          object: "",
          details: [listDetail(items)],
        };
      },
    },
  },
  ask_user: {
    default: {
      action: "提问",
      icon: "circle-help",
      bucket: "ask_user",
      unit: "个问题",
      build(payload) {
        const questions = readAskUserQuestions(payload);
        const preview = askUserPreviewFromQuestions(questions);
        const answers = askUserAnswerItems(payload);
        const questionItems = questions.map((question, index) =>
          askUserQuestionTitle(question, index)
        );
        const result = readAskUserResult(payload);
        const hasResult = Object.keys(result).length > 0;
        return {
          object: preview,
          preview,
          count: Math.max(1, questions.length),
          details: [
            answers.length ? listDetail(answers, true) : listDetail(questionItems, true),
            askUserCancelled(payload)
              ? markdownDetail("用户取消了提问。", "muted", true)
              : null,
            hasResult ? null : codeDetail("OUTPUT", pick(payload, ["output"])),
          ],
        };
      },
    },
  },
  tool: {
    default: {
      action: "调用工具",
      icon: "wrench",
      bucket: "tool",
      unit: "个工具",
      objectInLabel: true,
      build(payload) {
        const toolName = compactLine(pick(payload, ["toolName"]), 200);
        return {
          object: toolName,
          details: [
            fieldsDetail([displayField("工具", toolName)]),
            codeDetail("INPUT", pick(payload, ["input"])),
            codeDetail("OUTPUT", pick(payload, ["output"])),
          ],
        };
      },
    },
  },
};

export const LILIA_TOOL_KINDS = Object.freeze(Object.keys(LILIA_TOOL_REGISTRY));

export function getLiliaToolRule(kind, subkind) {
  const slot = LILIA_TOOL_REGISTRY[kind];
  if (!slot) return null;
  if (subkind && slot.subkinds?.[subkind]) return slot.subkinds[subkind];
  return slot.default || null;
}

export function deriveLiliaToolDisplay({ kind, subkind, payload, title }) {
  const rule = getLiliaToolRule(kind, subkind);
  if (!rule) return null;
  const safePayload = readRecord(payload);
  const built = rule.build(safePayload) ?? {};
  const object = compactLine(built.object, 1200);
  const details = Array.isArray(built.details)
    ? built.details.filter((d) => d !== null && d !== undefined)
    : [];
  const groupKey = subkind ? `${kind}:${subkind}` : `kind:${kind}`;
  const count = typeof built.count === "number" && Number.isFinite(built.count) && built.count > 0
    ? built.count
    : 1;
  return {
    icon: rule.icon,
    label: built.label,
    action: rule.action,
    object,
    objectInLabel: rule.objectInLabel === true ? true : undefined,
    preview: built.preview ?? object,
    details: details.length ? details : undefined,
    group: {
      key: groupKey,
      bucket: rule.bucket,
      unit: rule.unit,
      count,
    },
    defaultExpanded: built.defaultExpanded,
  };
}

function buildFileChangeDisplay(payload) {
  const path = compactLine(pick(payload, ["path"]), 1200);
  return {
    object: path,
    details: [fieldsDetail([displayField("文件", path)])],
  };
}

function buildSearchDisplay(payload) {
  const query = compactLine(pick(payload, ["query"]), 1200);
  return {
    object: query,
    details: [
      fieldsDetail([
        displayField("查询", query),
        displayField("path", pick(payload, ["path"])),
        displayField("glob", pick(payload, ["glob"])),
      ]),
    ],
  };
}
