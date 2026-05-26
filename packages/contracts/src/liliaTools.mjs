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
// | plan          | plan           | —                                             | —                   |
// | todo_list     | items[]        | —                                             | —                   |
// | tool          | toolName       | input / output                                | —（兜底）           |
//
// 派生 display 走 `deriveLiliaToolDisplay({kind, subkind, payload, ...})` —— 渲染时
// 现算的视图缓存，绝不持久化到 DB。改派生规则可即时影响历史事件。
//
// 这是 .mjs 而非 .ts：runner 由 Tauri 直接 `node agent-runner.mjs` 拉起，
// 不经过任何构建步骤；TS 端通过同目录 liliaTools.d.mts 拿到类型。

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

// ---------- display 派生规则（按 kind + subkind 分发） ----------

/**
 * 注册表：kind → {default, subkinds?}。每条规则给出渲染参数：
 *  - action: 泛指动词（"运行"/"读取"/"修改"...）
 *  - icon:   lucide 图标名
 *  - bucket: 折叠分组桶
 *  - unit:   折叠分组单位
 *  - objectInLabel: 是否把 object 拼进标题（true → "已调用工具 Edit"）
 *  - build:  (payload, ctx) => 返回 { object, details, preview? }
 */
const LILIA_TOOL_REGISTRY = {
  command: {
    default: {
      action: "运行",
      icon: "terminal",
      bucket: "command",
      unit: "条命令",
      build(payload) {
        const command = compactLine(pick(payload, ["command"]), 1200);
        const output = readFirstString(payload, ["output", "stdout"], 6000);
        const stderr = readFirstString(payload, ["stderr"], 6000);
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
        const plan = readFirstString(payload, ["plan"], 6000);
        return {
          object: "",
          details: [markdownDetail(plan)],
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

/** 工具范畴的 kind 列表：deriveTimelineDisplay 仅对它们查工具规则。 */
export const LILIA_TOOL_KINDS = Object.freeze(Object.keys(LILIA_TOOL_REGISTRY));

/** 按 kind+subkind 取规则；找不到返回 null。 */
export function getLiliaToolRule(kind, subkind) {
  const slot = LILIA_TOOL_REGISTRY[kind];
  if (!slot) return null;
  if (subkind && slot.subkinds?.[subkind]) return slot.subkinds[subkind];
  return slot.default || null;
}

/**
 * 按协议派生 display。入参：{kind, subkind?, payload, title}
 * 返回 AgentTimelineDisplay 形状的对象；找不到规则返回 null。
 *
 * 派生器只看 payload，不依赖事件生产方写入的 summary / title 字段
 * （title 仅用于 group key 兜底）。
 */
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
  return {
    icon: rule.icon,
    action: rule.action,
    object,
    objectInLabel: rule.objectInLabel === true ? true : undefined,
    preview: built.preview ?? object,
    details: details.length ? details : undefined,
    group: {
      key: groupKey,
      bucket: rule.bucket,
      unit: rule.unit,
      count: 1,
    },
  };
}

// ---------- build helper（被多个 subkind 共用） ----------

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
