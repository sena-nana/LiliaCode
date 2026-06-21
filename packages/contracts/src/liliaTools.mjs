// 这是 .mjs 而非 .ts：runner 由 Tauri 直接 `node agent-runner.mjs` 拉起。
import {
  compactLine,
  isRecord,
  parseRecordJson,
  pick,
  readFirstString,
  readFirstText,
  readRecord,
  shortText,
  stringOrNull,
} from "./toolUtils.mjs";
import {
  architectureChangeCompactLabel,
  readArchitectureChangeRecords,
} from "./architectureDisplay.mjs";
import {
  ARCHITECTURE_INTERACTION_KIND,
  ASK_USER_INTERACTION_KIND,
} from "./agentInteractionContract.mjs";
import {
  TIMELINE_DISPLAY_ALLOWED_PROMPT_TEXT_LIMIT,
  TIMELINE_DISPLAY_ASK_USER_HEADER_TEXT_LIMIT,
  TIMELINE_DISPLAY_COMMAND_INLINE_THRESHOLD,
  TIMELINE_DISPLAY_DETAIL_TEXT_LIMIT,
  TIMELINE_DISPLAY_FILE_CHANGE_PATH_TEXT_LIMIT,
  TIMELINE_DISPLAY_INLINE_TEXT_LIMIT,
  TIMELINE_DISPLAY_SHORT_TEXT_LIMIT,
  TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT,
  TIMELINE_DISPLAY_TINY_TEXT_LIMIT,
} from "./timelineContract.mjs";

export {
  compactLine,
  isRecord,
  pick,
  readFirstString,
  readFirstText,
  readRecord,
};

export function displayField(label, value) {
  const text = compactLine(value, TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT);
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
    content: shortText(text.trim(), TIMELINE_DISPLAY_DETAIL_TEXT_LIMIT),
    language: language || null,
  };
}

export function markdownDetail(content, tone = "default", singleLine = false) {
  const text = stringOrNull(content);
  if (!text || !text.trim()) return null;
  return {
    type: "markdown",
    content: shortText(text.trim(), TIMELINE_DISPLAY_DETAIL_TEXT_LIMIT),
    tone,
    singleLine,
  };
}

export function listDetail(items, ordered = false) {
  const normalized = (Array.isArray(items) ? items : [])
    .map((item) => {
      if (typeof item === "string") return { text: compactLine(item, TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT) };
      if (!isRecord(item)) return null;
      const text = readFirstString(
        item,
        ["text", "content", "title", "summary"],
        TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT,
      );
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
      const text = readFirstString(
        item,
        ["text", "content", "title", "description"],
        TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT,
      );
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
  const header = compactLine(pick(question, ["header"]), TIMELINE_DISPLAY_ASK_USER_HEADER_TEXT_LIMIT);
  const text = compactLine(
    pick(question, ["question", "title", "text"]),
    TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT,
  );
  if (header && text) return `${header} · ${text}`;
  return text || header || `问题 ${index + 1}`;
}

function askUserPreviewFromQuestions(questions) {
  if (!questions.length) return "用户提问";
  const title = askUserQuestionTitle(questions[0], 0);
  return questions.length > 1 ? `${title} 等 ${questions.length} 个问题` : title;
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
    const id = compactLine(pick(question, ["id"]), TIMELINE_DISPLAY_SHORT_TEXT_LIMIT);
    const text = compactLine(
      pick(question, ["question", "title", "text"]),
      TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT,
    );
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
    ? value.map((item) => compactLine(item, TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT)).filter(Boolean).join("、")
    : compactLine(value, TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT);
  const noteText = compactLine(pick(answerRecord, ["notes"]), TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT);
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
        : compactLine(key, TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT) || `问题 ${index + 1}`;
      const annotation = readRecord(annotations[key]);
      const valueText = formatAskUserAnswerValue(answer);
      const noteText = compactLine(pick(annotation, ["notes"]), TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT);
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

function architectureChangeStatusLabel(payload, status) {
  if (payload.status === "applied" || status === "success") return "已应用架构变更";
  if (payload.status === "rejected" || status === "cancelled" || status === "skipped") {
    return "已拒绝架构变更";
  }
  if (payload.requiresConfirmation === true || status === "requires_action") {
    return "等待确认架构变更";
  }
  return "提出架构变更";
}

const LILIA_TOOL_REGISTRY = {
  command: {
    default: {
      action: "运行",
      icon: "terminal",
      bucket: "command",
      unit: "条命令",
      build: buildCommandDisplay,
    },
    subkinds: {
      lilia_edit_exec: {
        action: "执行已编辑命令",
        icon: "terminal",
        bucket: "command",
        unit: "条命令",
        build(payload, status) {
          return buildCommandDisplay(payload, status, {
            commandKeys: ["modifiedCommand", "command"],
            includeEditedCommands: true,
          });
        },
      },
    },
  },
  file_read: {
    default: {
      action: "读取",
      icon: "book-open",
      bucket: "file",
      unit: "个文件",
      build(payload, status) {
        const path = compactLine(pick(payload, ["path"]), TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT);
        return {
          object: path,
          details: [errorOutputDetail(payload, status)],
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
        build: buildFileChangeDisplay,
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
        build: buildFileChangeDisplay,
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
        build: buildSearchDisplay,
      },
    },
  },
  web_fetch: {
    default: {
      action: "抓取网页",
      icon: "globe",
      bucket: "search",
      unit: "次搜索",
      build(payload, status) {
        const url = compactLine(pick(payload, ["url"]), TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT);
        return {
          object: url,
          details: [errorOutputDetail(payload, status)],
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
        const agentType = compactLine(pick(payload, ["agentType"]), TIMELINE_DISPLAY_SHORT_TEXT_LIMIT);
        const description = compactLine(pick(payload, ["description"]), TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT);
        const prompt = compactLine(pick(payload, ["prompt"]), TIMELINE_DISPLAY_DETAIL_TEXT_LIMIT);
        const result = compactLine(pick(payload, ["result"]), TIMELINE_DISPLAY_DETAIL_TEXT_LIMIT);
        return {
          object: agentType,
          details: [
            markdownDetail(description, "default"),
            !description && !result ? markdownDetail(prompt, "default") : null,
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
        const plan = readFirstText(payload, ["plan"], TIMELINE_DISPLAY_DETAIL_TEXT_LIMIT);
        const revisionRequest = readFirstText(payload, ["revisionRequest"], TIMELINE_DISPLAY_DETAIL_TEXT_LIMIT);
        const preview = revisionRequest
          ? `修改要求：${compactLine(revisionRequest, TIMELINE_DISPLAY_INLINE_TEXT_LIMIT)}`
          : compactLine(plan, TIMELINE_DISPLAY_INLINE_TEXT_LIMIT);
        const approved = payload.approved;
        const allowedPrompts = (Array.isArray(payload.allowedPrompts) ? payload.allowedPrompts : [])
          .filter(isRecord)
          .map((item) => {
            const tool = compactLine(pick(item, ["tool"]), TIMELINE_DISPLAY_TINY_TEXT_LIMIT);
            const prompt = compactLine(pick(item, ["prompt"]), TIMELINE_DISPLAY_ALLOWED_PROMPT_TEXT_LIMIT);
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
  [ASK_USER_INTERACTION_KIND]: {
    default: {
      action: "提问",
      icon: "circle-help",
      bucket: ASK_USER_INTERACTION_KIND,
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
  [ARCHITECTURE_INTERACTION_KIND]: {
    default: {
      action: "更新架构",
      icon: "git-branch",
      bucket: "architecture",
      unit: "次架构变更",
      build(payload, status) {
        const changes = readArchitectureChangeRecords(payload);
        const reason = readFirstText(payload, ["reason"], TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT);
        const versionText = [
          payload.beforeVersion !== undefined ? `v${payload.beforeVersion}` : "",
          payload.afterVersion !== undefined && payload.afterVersion !== null
            ? `v${payload.afterVersion}`
            : "",
        ].filter(Boolean).join(" -> ");
        return {
          object: reason,
          label: architectureChangeStatusLabel(payload, status),
          preview: reason || changes.map(architectureChangeCompactLabel).slice(0, 2).join("；"),
          count: Math.max(1, changes.length),
          details: [
            markdownDetail(reason, "default", true),
            fieldsDetail([
              displayField("version", versionText),
              displayField("permission", pick(payload, ["permission"])),
              displayField("project", pick(payload, ["projectId"])),
            ]),
            changes.length ? listDetail(changes.map(architectureChangeCompactLabel), true) : null,
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
        const toolName = compactLine(pick(payload, ["toolName"]), TIMELINE_DISPLAY_SHORT_TEXT_LIMIT);
        return {
          object: toolName,
          details: [
            codeDetail("INPUT", pick(payload, ["input"])),
            codeDetail("OUTPUT", pick(payload, ["output"])),
          ],
        };
      },
    },
    subkinds: {
      hook: {
        action: "运行 Hook",
        icon: "hook",
        bucket: "tool",
        unit: "个 Hook",
        objectInLabel: true,
        build(payload) {
          const hookName = compactLine(
            pick(payload, ["hookName", "toolName", "name"]),
            TIMELINE_DISPLAY_SHORT_TEXT_LIMIT,
          );
          const hookEvent = compactLine(
            pick(payload, ["hookEvent", "event"]),
            TIMELINE_DISPLAY_SHORT_TEXT_LIMIT,
          );
          const output = readFirstText(
            payload,
            ["output", "stdout", "result", "response"],
            TIMELINE_DISPLAY_DETAIL_TEXT_LIMIT,
          );
          const stderr = readFirstText(
            payload,
            ["stderr", "error", "message"],
            TIMELINE_DISPLAY_DETAIL_TEXT_LIMIT,
          );
          return {
            object: hookName || hookEvent,
            details: [
              fieldsDetail([
                displayField("event", hookEvent),
                displayField("exit", pick(payload, ["exitCode", "exit"])),
              ]),
              codeDetail("INPUT", pick(payload, ["input", "arguments", "args", "parameters"])),
              codeDetail(stderr ? "ERROR / OUTPUT" : "OUTPUT", output || stderr),
            ],
          };
        },
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

export function deriveLiliaToolDisplay({ kind, subkind, payload, title, status }) {
  const rule = getLiliaToolRule(kind, subkind);
  if (!rule) return null;
  const safePayload = readRecord(payload);
  const built = rule.build(safePayload, status) ?? {};
  const object = compactLine(built.object, TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT);
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

function buildCommandDisplay(payload, status, options = {}) {
  const commandKeys = Array.isArray(options.commandKeys) ? options.commandKeys : ["command"];
  const command = compactLine(pick(payload, commandKeys), TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT);
  const output = readFirstText(
    payload,
    ["output", "aggregatedOutput", "stdout"],
    TIMELINE_DISPLAY_DETAIL_TEXT_LIMIT,
  );
  const stderr = readFirstText(payload, ["stderr", "error"], TIMELINE_DISPLAY_DETAIL_TEXT_LIMIT);
  const combinedOutput = output && stderr && !output.includes(stderr)
    ? `${output}\n${stderr}`.trim()
    : output || stderr;
  const shouldShowCommand = command.length > TIMELINE_DISPLAY_COMMAND_INLINE_THRESHOLD ||
    Boolean(output || stderr || options.includeEditedCommands);
  return {
    object: command,
    details: [
      fieldsDetail([
        displayField("cwd", pick(payload, ["cwd"])),
        displayField("exit", pick(payload, ["exit", "exitCode"])),
        displayField("duration", pick(payload, ["duration", "durationMs"])),
        displayField("approval", pick(payload, ["approvalId"])),
        displayField("owner", pick(payload, ["executionOwner"])),
      ]),
      options.includeEditedCommands
        ? codeDetail("ORIGINAL COMMAND", pick(payload, ["originalCommand"]), "shell")
        : null,
      options.includeEditedCommands
        ? codeDetail("MODIFIED COMMAND", pick(payload, ["modifiedCommand", "command"]), "shell")
        : null,
      !options.includeEditedCommands && shouldShowCommand
        ? codeDetail("COMMAND", command, "shell")
        : null,
      codeDetail(stderr ? "ERROR / OUTPUT" : "OUTPUT", combinedOutput),
    ],
  };
}

function buildFileChangeDisplay(payload, status) {
  const path = compactLine(pick(payload, ["path"]), TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT);
  const changes = readFileChanges(payload);
  const changeItems = changes.length > 1
    ? listDetail(changes.map((change) => `${change.kind} ${change.path}`))
    : null;
  return {
    object: path || changes[0]?.path || "",
    details: [changeItems, errorOutputDetail(payload, status)],
  };
}

function buildSearchDisplay(payload, status) {
  const query = compactLine(pick(payload, ["query"]), TIMELINE_DISPLAY_SUMMARY_TEXT_LIMIT);
  return {
    object: query,
    details: [errorOutputDetail(payload, status)],
  };
}

export function isFailureStatus(status) {
  return status === "failed" ||
    status === "error" ||
    status === "cancelled";
}

export function errorOutputDetail(payload, status) {
  if (!isFailureStatus(status)) return null;
  const output = readFirstText(payload, [
    "aggregatedOutput",
    "combinedOutput",
    "outputText",
    "stderr",
    "errorOutput",
    "stdout",
    "output",
    "error",
    "message",
  ], TIMELINE_DISPLAY_DETAIL_TEXT_LIMIT);
  return codeDetail("ERROR / OUTPUT", output);
}

export function readFileChanges(payload) {
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
    .map((change) => {
      if (!isRecord(change)) return null;
      const path = readFirstString(
        change,
        ["path", "filePath", "relativePath", "targetPath", "name"],
        TIMELINE_DISPLAY_FILE_CHANGE_PATH_TEXT_LIMIT,
      );
      if (!path) return null;
      return {
        kind: readFirstString(
          change,
          ["kind", "operation", "type", "status"],
          TIMELINE_DISPLAY_TINY_TEXT_LIMIT,
        ) || "update",
        path,
      };
    })
    .filter((change) => change !== null);
}
