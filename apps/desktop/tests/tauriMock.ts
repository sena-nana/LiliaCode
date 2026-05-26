import { vi } from "vitest";

interface ProjectRow {
  id: string;
  name: string;
  cwd: string | null;
  sessionCount: number;
  sortOrder: number;
  pinned: boolean;
}

interface TaskRow {
  id: string;
  projectId: string | null;
  sessionId: string;
  title: string;
  status: string;
  createdAt: number;
  parentId: string | null;
  dependsOn: string[];
  sortOrder: number;
  pinned: boolean;
}

interface AgentTimelineEvent {
  id: string;
  taskId: string;
  turnId: string | null;
  backend: "claude" | "codex";
  kind: string;
  status: string;
  title: string;
  summary: string | null;
  payload: unknown;
  display: AgentTimelineDisplay;
  createdAt: number;
  updatedAt: number;
  order: number;
}

type AgentTimelineDisplay = Record<string, unknown>;
type AgentTimelineDisplayDetail = Record<string, unknown>;

const baseProjects: ProjectRow[] = [
  {
    id: "lilia",
    name: "Lilia",
    cwd: "D:\\PROJECT\\workspace\\Lilia",
    sessionCount: 2,
    sortOrder: 0,
    pinned: false,
  },
  {
    id: "tools",
    name: "工具箱",
    cwd: "D:\\PROJECT\\workspace\\tools",
    sessionCount: 1,
    sortOrder: 1,
    pinned: false,
  },
];

const baseTasks: TaskRow[] = [
  {
    id: "t-001",
    projectId: "lilia",
    sessionId: "0192-lilia-0001",
    title: "接入 Claude Code 会话发现",
    status: "done",
    createdAt: 1000,
    parentId: null,
    dependsOn: [],
    sortOrder: 0,
    pinned: false,
  },
  {
    id: "t-002",
    projectId: "lilia",
    sessionId: "0192-lilia-0002",
    title: "打通 tsconfig paths 搜索",
    status: "running",
    createdAt: 2000,
    parentId: null,
    dependsOn: ["t-001"],
    sortOrder: 1,
    pinned: false,
  },
  {
    id: "t-003",
    projectId: "tools",
    sessionId: "0192-tools-0001",
    title: "整理窗口快捷键",
    status: "waiting",
    createdAt: 3000,
    parentId: null,
    dependsOn: [],
    sortOrder: 0,
    pinned: false,
  },
  {
    id: "o-001",
    projectId: null,
    sessionId: "0192-orphan-0001",
    title: "随手问问 Claude：tsconfig paths",
    status: "running",
    createdAt: 4000,
    parentId: null,
    dependsOn: [],
    sortOrder: 0,
    pinned: false,
  },
];

let projects: ProjectRow[] = [];
let tasks: TaskRow[] = [];
let timelineEvents: Record<string, AgentTimelineEvent[]> = {};
let chatRunning: Record<string, boolean> = {};
let chatQueued: Record<string, Array<Record<string, unknown>>> = {};
let eventHandlers: Record<string, Array<(event: { payload: unknown }) => void>> = {};

function cloneProject(row: ProjectRow): ProjectRow {
  return { ...row };
}

function cloneTask(row: TaskRow): TaskRow {
  return { ...row, dependsOn: [...row.dependsOn] };
}

function refreshSessionCounts() {
  projects = projects.map((project) => ({
    ...project,
    sessionCount: tasks.filter((task) =>
      task.projectId === project.id && task.status !== "cancelled"
    ).length,
  }));
}

export function resetTauriMockData() {
  projects = baseProjects.map(cloneProject);
  tasks = baseTasks.map(cloneTask);
  timelineEvents = {
    "t-002": [
      {
        id: "tl-existing",
        taskId: "t-002",
        turnId: "turn-existing",
        backend: "claude",
        kind: "reasoning",
        status: "info",
        title: "历史思考摘要",
        summary: "从持久化时间线恢复的公开摘要。",
        payload: { source: "mock" },
        display: {
          icon: "reasoning",
          action: "思考",
          object: "历史思考摘要",
          preview: "从持久化时间线恢复的公开摘要。",
        },
        createdAt: 1500,
        updatedAt: 1500,
        order: 0,
      },
    ],
  };
  chatRunning = {};
  chatQueued = {};
  eventHandlers = {};
  refreshSessionCounts();
  mockInvoke.mockClear();
  mockListen.mockClear();
}

export function emitTauriEvent(event: string, payload: unknown) {
  for (const handler of eventHandlers[event] ?? []) {
    handler({ payload });
  }
}

export function completeMockAgentTurn(taskId: string) {
  emitTauriEvent("chat:done", {
    taskId,
    sessionId: `mock-${taskId}`,
    subtype: null,
  });
  const [next, ...rest] = chatQueued[taskId] ?? [];
  if (next) {
    chatQueued[taskId] = rest;
    chatRunning[taskId] = true;
    const queuedMessage = (timelineEvents[taskId] ?? []).find((event) => {
      const payload = event.payload as Record<string, unknown> | null;
      return event.kind === "message" && payload?.queued === true;
    });
    if (queuedMessage) {
      emitMockTimelineEvent(taskId, {
        ...queuedMessage,
        status: "success",
        payload: {
          ...(queuedMessage.payload as Record<string, unknown>),
          queued: false,
        },
        updatedAt: Date.now(),
      });
    }
    queueMicrotask(() => {
      emitTauriEvent("chat:turn-started", {
        taskId,
        queuedCount: rest.length,
      });
    });
  } else {
    chatRunning[taskId] = false;
  }
}

export function emitMockTimelineEvent(
  taskId: string,
  patch: Partial<AgentTimelineEvent> = {},
) {
  const base = {
    id: patch.id ?? `tl-${Date.now()}`,
    taskId,
    turnId: patch.turnId ?? "turn-live",
    backend: patch.backend ?? "claude",
    kind: patch.kind ?? "command",
    status: patch.status ?? "running",
    title: patch.title ?? "实时命令",
    summary: patch.summary ?? "正在运行命令",
    payload: patch.payload ?? { command: "yarn test" },
    createdAt: patch.createdAt ?? Date.now(),
    updatedAt: patch.updatedAt ?? Date.now(),
    order: patch.order ?? (timelineEvents[taskId]?.length ?? 0),
  } satisfies Omit<AgentTimelineEvent, "display">;
  const event: AgentTimelineEvent = {
    ...base,
    display: patch.display ?? mockTimelineDisplay(base),
  };
  timelineEvents[taskId] = [
    ...(timelineEvents[taskId] ?? []).filter((item) => item.id !== event.id),
    event,
  ];
  emitTauriEvent("agent:timeline", event);
  return event;
}

export function seedMockChatMessages(taskId: string, messages: unknown[]) {
  const messageEvents = messages
    .map((message, index): AgentTimelineEvent | null => {
      if (!message || typeof message !== "object" || Array.isArray(message)) return null;
      const row = message as Record<string, unknown>;
      if (row.role !== "user" && row.role !== "system") return null;
      const id = String(row.id ?? `message-${index}`);
      const content = String(row.content ?? "");
      const createdAt = typeof row.createdAt === "number" ? row.createdAt : Date.now();
      const title = row.role === "system" ? "系统消息" : "用户输入";
      return {
        id,
        taskId,
        turnId: null,
        backend: "claude",
        kind: "message",
        status: "success",
        title,
        summary: content,
        payload: {
          role: row.role,
          content,
          queued: false,
        },
        display: {
          icon: "message",
          label: title,
          preview: content,
        },
        createdAt,
        updatedAt: createdAt,
        order: 0,
      };
    })
    .filter((event): event is AgentTimelineEvent => event !== null);
  timelineEvents[taskId] = [
    ...messageEvents,
    ...(timelineEvents[taskId] ?? []).filter((event) => event.kind !== "message"),
  ];
}

export const mockListen = vi.fn(async (
  event: string,
  handler: (event: { payload: unknown }) => void,
) => {
  eventHandlers[event] = [...(eventHandlers[event] ?? []), handler];
  return async () => {
    eventHandlers[event] = (eventHandlers[event] ?? []).filter((h) => h !== handler);
  };
});

export const mockInvoke = vi.fn(async (cmd: string, args: Record<string, unknown> = {}) => {
  switch (cmd) {
    case "project_list":
      refreshSessionCounts();
      return projects
        .map(cloneProject)
        .sort((a, b) => a.sortOrder - b.sortOrder);

    case "project_get": {
      const id = String(args.id);
      refreshSessionCounts();
      return projects.find((project) => project.id === id) ?? null;
    }

    case "project_create": {
      const name = String(args.name || "未命名项目");
      const cwd = typeof args.cwd === "string" ? args.cwd : null;
      const project: ProjectRow = {
        id: `p-${projects.length + 1}`,
        name,
        cwd,
        sessionCount: 0,
        sortOrder: projects.length,
        pinned: false,
      };
      projects.push(project);
      return cloneProject(project);
    }

    case "project_remove": {
      const id = String(args.id);
      const before = projects.length;
      projects = projects.filter((project) => project.id !== id);
      tasks = tasks.map((task) =>
        task.projectId === id ? { ...task, projectId: null } : task
      );
      refreshSessionCounts();
      return projects.length !== before;
    }

    case "task_list": {
      const projectId = args.projectId as string | null | undefined;
      return tasks
        .filter((task) => task.projectId === (projectId ?? null))
        .map(cloneTask)
        .sort((a, b) =>
          Number(b.pinned) - Number(a.pinned) || a.sortOrder - b.sortOrder
        );
    }

    case "task_toggle_pin": {
      const id = String(args.id);
      let pinned = false;
      tasks = tasks.map((task) => {
        if (task.id !== id) return task;
        pinned = !task.pinned;
        return { ...task, pinned };
      });
      return pinned;
    }

    case "task_archive_project": {
      const projectId = String(args.projectId);
      const count = tasks.filter((task) => task.projectId === projectId).length;
      tasks = tasks.filter((task) => task.projectId !== projectId);
      refreshSessionCounts();
      return count;
    }

    case "chat_check_env":
      return {
        nodeAvailable: true,
        codexCliAvailable: true,
        ccSwitch: {
          reachable: true,
          baseUrl: "http://127.0.0.1:15721",
        },
        routerModes: {
          claude: "cc-switch",
          codex: "cc-switch",
        },
        backends: {
          claude: {
            backend: "claude",
            hasApiKey: true,
            connectionMode: "cc-switch",
            effectiveUrl: "http://127.0.0.1:15721",
          },
          codex: {
            backend: "codex",
            hasApiKey: true,
            connectionMode: "cc-switch",
            effectiveUrl: "http://127.0.0.1:15721",
          },
        },
      };

    case "agent_timeline_list": {
      const taskId = String(args.taskId);
      return timelineEvents[taskId] ?? [];
    }

    case "agent_timeline_clear_task": {
      const taskId = String(args.taskId);
      const count = timelineEvents[taskId]?.length ?? 0;
      timelineEvents[taskId] = [];
      return count;
    }

    case "chat_get_composer_state": {
      const taskId = String(args.taskId);
      return {
        taskId,
        backend: "claude",
        model: "claude-sonnet-4-6",
        branch: "main",
        permission: "ask",
      };
    }

    case "chat_set_composer_state":
      return undefined;

    case "chat_list_models": {
      const backend = String(args.backend || "claude");
      return [
        {
          id: backend === "codex" ? "gpt-5-codex" : "claude-sonnet-4-6",
          label: backend === "codex" ? "GPT-5 Codex" : "Sonnet 4.6",
          backend,
        },
      ];
    }

    case "chat_list_branches":
      return [{ name: "main", current: true }];

    case "todo_list":
      return [];

    case "chat_send_message": {
      const taskId = String(args.taskId);
      const content = String(args.content);
      const queued = chatRunning[taskId] === true;
      const message = {
        id: `u-${(timelineEvents[taskId]?.filter((event) => event.kind === "message").length ?? 0) + 1}`,
        taskId,
        role: "user",
        content,
        createdAt: Date.now(),
      };
      emitMockTimelineEvent(taskId, {
        id: message.id,
        turnId: null,
        kind: "message",
        status: queued ? "pending" : "success",
        title: "用户输入",
        summary: content,
        payload: {
          role: "user",
          content,
          queued,
        },
        createdAt: message.createdAt,
        updatedAt: message.createdAt,
        order: 0,
      });
      if (queued) {
        chatQueued[taskId] = [...(chatQueued[taskId] ?? []), args];
        return {
          message,
          dispatch: "queued",
          queuedCount: chatQueued[taskId].length,
        };
      }
      chatRunning[taskId] = true;
      queueMicrotask(() => {
        emitTauriEvent("chat:turn-started", {
          taskId,
          queuedCount: 0,
        });
      });
      return {
        message,
        dispatch: "started",
        queuedCount: 0,
      };
    }

    case "plugin:event|listen": {
      const event = String(args.event);
      const handlerId = Number(args.handler);
      return handlerId || 1;
    }

    default:
      throw new Error(`未配置的 Tauri mock 命令：${cmd}`);
  }
});

resetTauriMockData();

function mockTimelineDisplay(
  event: Pick<AgentTimelineEvent, "kind" | "title" | "summary" | "payload">,
): AgentTimelineDisplay {
  const payload = readRecord(event.payload);
  const title = event.title.trim() || event.kind;
  const preview = event.summary?.trim() || title;

  if (event.kind === "message") {
    const role = typeof payload.role === "string" ? payload.role : "user";
    const content = readString(payload, ["content"]) || event.summary || "";
    return {
      icon: "message-square",
      label: role === "system" ? "系统消息" : role === "assistant" ? "Assistant" : "用户输入",
      preview: content,
    };
  }

  if (event.kind === "reasoning") {
    return {
      action: "思考",
      object: title,
      preview,
    };
  }

  if (event.kind === "command") {
    const command = readString(payload, ["command", "cmd", "shellCommand"]) || title;
    return commandDisplay(command, preview, payload);
  }

  if (event.kind === "file_change") {
    const changes = readArray(payload.changes);
    return fileDisplay(title, preview, Math.max(1, changes.length), payload);
  }

  if (event.kind === "plan") {
    return {
      icon: "list-ordered",
      action: "制定计划",
      object: title,
      preview,
      details: markdownDetails(readString(payload, ["plan", "content", "text"]) || preview),
      group: { key: "kind:plan", bucket: "plan", unit: "项计划", count: 1 },
    };
  }

  if (event.kind === "todo_list") {
    return {
      icon: "list-checks",
      action: "更新待办",
      object: title,
      preview,
      group: { key: "kind:todo", bucket: "todo", unit: "次待办", count: 1 },
    };
  }

  if (event.kind === "error") {
    const message = readString(payload, ["message", "error"]) || preview;
    return {
      icon: "alert-triangle",
      label: "错误",
      preview: message,
      details: [{ type: "line", text: message, tone: "muted" }],
      group: { key: "kind:error", bucket: "error", unit: "个错误", count: 1 },
    };
  }

  if (event.kind === "mcp") {
    return {
      icon: "plug",
      action: "调用",
      object: title,
      preview,
      group: { key: "kind:mcp", bucket: "mcp", unit: "个 MCP", count: 1 },
    };
  }

  if (event.kind === "web_search") {
    return {
      icon: "search",
      action: "搜索",
      object: title,
      preview,
      group: { key: "kind:web_search", bucket: "web_search", unit: "次搜索", count: 1 },
    };
  }

  if (event.kind === "subagent") {
    return {
      icon: "bot",
      action: "运行子代理",
      object: title,
      preview,
      group: { key: "kind:subagent", bucket: "subagent", unit: "个子代理", count: 1 },
    };
  }

  if (event.kind === "tool") {
    return mockToolDisplay(title, preview, payload);
  }

  return {
    action: "处理",
    object: title,
    preview,
    group: { key: `kind:${event.kind}`, bucket: "other", unit: "项", count: 1 },
  };
}

function mockToolDisplay(
  title: string,
  preview: string,
  payload: Record<string, unknown>,
): AgentTimelineDisplay {
  const input = readRecord(payload.input);
  const toolName = readString(payload, ["toolName", "tool", "name"]) || title;
  const normalized = toolName.toLowerCase();

  if (normalized === "bash") {
    const command = readString(input, ["command"]) || readString(payload, ["command"]) || title;
    return commandDisplay(command, preview, payload);
  }

  if (["read", "edit", "write", "multiedit", "notebookedit"].includes(normalized)) {
    const path = readString(input, ["path", "filePath", "file_path", "file"]) ||
      readString(payload, ["path", "filePath", "file"]);
    return fileDisplay(path || title, preview, 1, payload);
  }

  if (normalized === "todowrite") {
    return {
      icon: "list-checks",
      action: "更新待办",
      object: title,
      preview,
      group: { key: "tool:todo", bucket: "todo", unit: "次待办", count: 1 },
    };
  }

  if (["websearch", "webfetch", "grep", "glob", "ls"].includes(normalized)) {
    return {
      icon: "search",
      action: "搜索",
      object: title,
      preview,
      group: { key: "tool:search", bucket: "web_search", unit: "次搜索", count: 1 },
    };
  }

  if (["task", "agent"].includes(normalized)) {
    return {
      icon: "bot",
      action: "运行子代理",
      object: title,
      preview,
      group: { key: "tool:subagent", bucket: "subagent", unit: "个子代理", count: 1 },
    };
  }

  return {
    icon: "wrench",
    action: "调用",
    object: title,
    preview,
    group: { key: "kind:tool", bucket: "tool", unit: "个工具", count: 1 },
  };
}

function commandDisplay(
  command: string,
  preview: string,
  payload: Record<string, unknown>,
): AgentTimelineDisplay {
  const details = [
    fieldsDetail([{ label: "命令", value: command }]),
    codeDetail("stdout", readString(payload, ["stdout", "output", "result"])),
    codeDetail("stderr", readString(payload, ["stderr", "error"])),
  ].filter((detail): detail is AgentTimelineDisplayDetail => detail !== null);
  return {
    icon: "terminal",
    action: "运行",
    object: command,
    preview,
    details,
    group: { key: "kind:command", bucket: "command", unit: "条命令", count: 1 },
  };
}

function fileDisplay(
  object: string,
  preview: string,
  count: number,
  payload: Record<string, unknown>,
): AgentTimelineDisplay {
  const changes = readArray(payload.changes)
    .map((change) => {
      const row = readRecord(change);
      return readString(row, ["path", "file", "filePath"]);
    })
    .filter((path) => path.trim());
  return {
    icon: "file-pen",
    action: "修改",
    object,
    preview,
    details: changes.length
      ? [{ type: "list", items: changes.map((path) => ({ text: path })) }]
      : undefined,
    group: { key: "kind:file", bucket: "file", unit: "个文件", count },
  };
}

function fieldsDetail(fields: Array<{ label: string; value: string }>): AgentTimelineDisplayDetail | null {
  const visible = fields.filter((field) => field.label.trim() && field.value.trim());
  return visible.length ? { type: "fields", fields: visible } : null;
}

function codeDetail(label: string, content: string): AgentTimelineDisplayDetail | null {
  return content.trim() ? { type: "code", label, content } : null;
}

function markdownDetails(content: string): AgentTimelineDisplayDetail[] | undefined {
  return content.trim() ? [{ type: "markdown", content }] : undefined;
}

function readRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function readArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function readString(payload: Record<string, unknown>, keys: string[]): string {
  for (const key of keys) {
    const value = payload[key];
    if (typeof value === "string" && value.trim()) return value;
    if (typeof value === "number" || typeof value === "boolean") return String(value);
  }
  return "";
}

