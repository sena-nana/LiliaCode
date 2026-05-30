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

/**
 * 与 @lilia/contracts 同形，display 已从持久层下线：前端用
 * `deriveTimelineDisplay()` 现算，mock 这里也只塞「事实」字段。
 */
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
  createdAt: number;
  updatedAt: number;
  turnSeq: number;
  intraTurnOrder: number;
}

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
let webviewDragDropHandlers: Array<(event: { payload: unknown }) => void> = [];
let projectPinUpdater: ((projectId: string, pinned: boolean) => void) | null = null;

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
        createdAt: 1500,
        updatedAt: 1500,
        turnSeq: 0,
        intraTurnOrder: 0,
      },
    ],
  };
  chatRunning = {};
  chatQueued = {};
  eventHandlers = {};
  webviewDragDropHandlers = [];
  refreshSessionCounts();
  mockInvoke.mockClear();
  mockListen.mockClear();
  mockGetCurrentWebview.mockClear();
}

export function emitTauriEvent(event: string, payload: unknown) {
  for (const handler of eventHandlers[event] ?? []) {
    handler({ payload });
  }
}

export function emitWebviewDragDropEvent(payload: unknown) {
  for (const handler of webviewDragDropHandlers) {
    handler({ payload });
  }
}

export function setMockProjectPinned(projectId: string, pinned: boolean) {
  projects = projects.map((project) =>
    project.id === projectId ? { ...project, pinned } : project
  );
  projectPinUpdater?.(projectId, pinned);
}

export function bindMockProjectPinUpdater(
  updater: ((projectId: string, pinned: boolean) => void) | null,
) {
  projectPinUpdater = updater;
}

export function mockProjectsForStore() {
  refreshSessionCounts();
  return projects.map((project) => ({
    id: project.id,
    name: project.name,
    cwd: project.cwd,
    sessionCount: project.sessionCount,
    pinned: project.pinned,
  }));
}

export function mockTasksByProjectForStore() {
  const byProject: Record<string, unknown[]> = {};
  for (const project of projects) {
    byProject[project.id] = tasks
      .filter((task) => task.projectId === project.id)
      .map(cloneTask)
      .sort((a, b) =>
        Number(b.pinned) - Number(a.pinned) || a.sortOrder - b.sortOrder
      )
      .map((task) => ({
        id: task.id,
        projectId: task.projectId ?? "",
        sessionId: task.sessionId,
        title: task.title,
        status: task.status,
        createdAt: task.createdAt,
        pinned: task.pinned,
        parentId: task.parentId,
        dependsOn: [...task.dependsOn],
      }));
  }
  return byProject;
}

export function mockOrphansForStore() {
  return tasks
    .filter((task) => task.projectId === null)
    .map(cloneTask)
    .sort((a, b) =>
      Number(b.pinned) - Number(a.pinned) || a.sortOrder - b.sortOrder
    )
    .map((task) => ({
      id: task.id,
      sessionId: task.sessionId,
      title: task.title,
      createdAt: task.createdAt,
      pinned: task.pinned,
    }));
}

export const mockGetCurrentWebview = vi.fn(() => ({
  onDragDropEvent: vi.fn(async (handler: (event: { payload: unknown }) => void) => {
    webviewDragDropHandlers = [...webviewDragDropHandlers, handler];
    return async () => {
      webviewDragDropHandlers = webviewDragDropHandlers.filter((h) => h !== handler);
    };
  }),
}));

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

function currentChatTurnId(taskId: string): string {
  const event = [...(timelineEvents[taskId] ?? [])].reverse().find((candidate) => {
    const payload = candidate.payload as Record<string, unknown> | null;
    return candidate.kind === "message" &&
      typeof candidate.turnId === "string" &&
      payload?.role === "user" &&
      payload?.queued !== true;
  });
  return event?.turnId ?? `turn-interrupted-${Date.now()}`;
}

export function emitMockTimelineEvent(
  taskId: string,
  patch: Partial<AgentTimelineEvent> = {},
) {
  const event: AgentTimelineEvent = {
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
    turnSeq: patch.turnSeq ?? 0,
    intraTurnOrder: patch.intraTurnOrder ?? (timelineEvents[taskId]?.length ?? 0),
  };
  timelineEvents[taskId] = [
    ...(timelineEvents[taskId] ?? []).filter((item) => item.id !== event.id),
    event,
  ];
  emitTauriEvent("agent:timeline", event);
  return event;
}

/**
 * Runner 在 `case "result"` 里 emit 的 `kind:"turn"` 终结事件——前端 UI 把这条
 * 当作"该 turn 已结束"的权威信号，进而把过程折叠到最终回复下。
 */
export function emitMockTurnCompleted(
  taskId: string,
  turnId: string,
  status: AgentTimelineEvent["status"] = "success",
  createdAt?: number,
) {
  const at = createdAt ?? Date.now();
  return emitMockTimelineEvent(taskId, {
    id: `tl-turn-complete-${turnId}`,
    kind: "turn",
    status,
    title: "Claude turn completed",
    summary: "",
    payload: { backend: "claude" },
    turnId,
    createdAt: at,
    updatedAt: at,
  });
}

export function seedMockChatMessages(taskId: string, messages: unknown[]) {
  const messageEvents = messages
    .map((message, index): AgentTimelineEvent | null => {
      if (!message || typeof message !== "object" || Array.isArray(message)) return null;
      const row = message as Record<string, unknown>;
      if (row.role !== "user" && row.role !== "system") return null;
      const id = String(row.id ?? `message-${index}`);
      const content = String(row.content ?? "");
      const attachments = Array.isArray(row.attachments) ? row.attachments : [];
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
          attachments,
          queued: false,
        },
        createdAt,
        updatedAt: createdAt,
        turnSeq: index,
        intraTurnOrder: 0,
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

    case "project_reorder": {
      const orderedIds = Array.isArray(args.orderedIds) ? args.orderedIds.map(String) : [];
      projects = projects.map((project) => {
        const index = orderedIds.indexOf(project.id);
        return index >= 0 ? { ...project, sortOrder: index } : project;
      });
      return undefined;
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

    case "task_promote": {
      const id = String(args.id);
      const projectId = typeof args.projectId === "string" ? args.projectId : null;
      const title = String(args.title ?? "新对话");
      const dependsOn = Array.isArray(args.dependsOn) ? args.dependsOn.map(String) : [];
      const sortOrder = tasks.filter((task) => task.projectId === projectId).length;
      const row: TaskRow = {
        id,
        projectId,
        sessionId: id,
        title,
        status: "running",
        createdAt: Date.now(),
        parentId: null,
        dependsOn,
        sortOrder,
        pinned: false,
      };
      tasks = [row, ...tasks.filter((task) => task.id !== id)];
      refreshSessionCounts();
      return cloneTask(row);
    }

    case "task_reorder": {
      const projectId = args.projectId as string | null | undefined;
      const targetProjectId = projectId ?? null;
      const orderedIds = Array.isArray(args.orderedIds) ? args.orderedIds.map(String) : [];
      tasks = tasks.map((task) => {
        if (task.projectId !== targetProjectId) return task;
        const index = orderedIds.indexOf(task.id);
        return index >= 0 ? { ...task, sortOrder: index } : task;
      });
      return undefined;
    }

    case "task_reparent": {
      const taskId = String(args.taskId);
      const newProjectId = args.newProjectId === null || args.newProjectId === undefined
        ? null
        : String(args.newProjectId);
      tasks = tasks.map((task) =>
        task.id === taskId
          ? { ...task, projectId: newProjectId, sortOrder: Number.MAX_SAFE_INTEGER }
          : task
      );
      return undefined;
    }

    case "task_archive": {
      const id = String(args.id);
      const before = tasks.length;
      tasks = tasks.filter((task) => task.id !== id);
      refreshSessionCounts();
      return tasks.length !== before;
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
        planMode: false,
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

    case "chat_respond_ask_user":
      return undefined;

    case "chat_respond_tool_consent":
      return undefined;

    case "chat_describe_attachments": {
      const paths = Array.isArray(args.paths) ? args.paths.map(String) : [];
      return paths.map((path, index) => {
        const name = path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
        return {
          id: `att-${index + 1}`,
          name,
          path,
          kind: path.includes(".") ? "file" : "directory",
          size: path.includes(".") ? 42 : null,
        };
      });
    }

    case "todo_list":
      return [];

    case "chat_send_message": {
      const taskId = String(args.taskId);
      const content = String(args.content);
      const attachments = Array.isArray(args.attachments) ? args.attachments : [];
      const queued = chatRunning[taskId] === true;
      const message = {
        id: `u-${(timelineEvents[taskId]?.filter((event) => event.kind === "message").length ?? 0) + 1}`,
        taskId,
        role: "user",
        content,
        attachments,
        createdAt: Date.now(),
      };
      const turnId = queued
        ? `turn-queued-${message.id}`
        : `turn-${message.id}`;
      emitMockTimelineEvent(taskId, {
        id: message.id,
        turnId,
        kind: "message",
        status: queued ? "pending" : "success",
        title: "用户输入",
        summary: content,
        payload: {
          role: "user",
          content,
          attachments,
          queued,
        },
        createdAt: message.createdAt,
        updatedAt: message.createdAt,
        turnSeq: 0,
        intraTurnOrder: 0,
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

    case "chat_interrupt_turn": {
      const taskId = String(args.taskId);
      const turnId = currentChatTurnId(taskId);
      const message = "用户打断了当前 Agent 运行";
      chatQueued[taskId] = [];
      if (chatRunning[taskId] === true) {
        emitMockTimelineEvent(taskId, {
          id: `tl-interrupted-${turnId}`,
          kind: "error",
          status: "error",
          title: "Agent 已打断",
          summary: message,
          payload: {
            backend: "claude",
            interrupted: true,
            message,
          },
          turnId,
        });
        chatRunning[taskId] = false;
        queueMicrotask(() => {
          emitTauriEvent("chat:done", {
            taskId,
            sessionId: null,
            subtype: null,
          });
        });
      }
      return undefined;
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
