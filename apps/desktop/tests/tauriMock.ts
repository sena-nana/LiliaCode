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

interface TodoRow {
  id: string;
  taskId: string;
  text: string;
  done: boolean;
  order: number;
  source: "lilia" | "agent";
  priority: "high" | "normal" | "low";
  guideStatus: "pending" | "queued" | "sent" | null;
  attachments: unknown[];
  createdAt: number;
  updatedAt: number;
}

interface AgentTodoInput {
  content?: unknown;
  text?: unknown;
  title?: unknown;
  description?: unknown;
  status?: unknown;
  completed?: unknown;
  done?: unknown;
  priority?: unknown;
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
let todosByTaskId: Record<string, TodoRow[]> = {};
let todoSeq = 0;
let chatRunning: Record<string, boolean> = {};
let chatQueued: Record<string, Array<Record<string, unknown>>> = {};
let clipboardFilePaths: string[] = [];
let clipboardImageSeq = 0;
let activeBackend: "claude" | "codex" = "claude";
let composerStateHandler: ((taskId: string) => unknown | Promise<unknown>) | null = null;
const baseClaudePlugins = [{
  scope: "user",
  name: "demo-plugin",
  description: "测试用 Claude plugin",
  version: "1.0.0",
  enabled: true,
  path: "C:\\Users\\mock\\.claude\\plugins\\demo-plugin",
}];
const baseClaudeMcpServers = [{
  name: "weather",
  command: "node",
  args: ["weather-mcp.js"],
  envKeys: ["WEATHER_TOKEN"],
  enabled: true,
}];
let claudePlugins = baseClaudePlugins.map((plugin) => ({ ...plugin }));
let claudeMcpServers = baseClaudeMcpServers.map((server) => ({
  ...server,
  args: [...server.args],
  envKeys: [...server.envKeys],
}));
let agentInteractionSettings = { nonInterruptMode: false, debug: false };
let eventHandlers: Record<string, Array<(event: { payload: unknown }) => void>> = {};
let webviewDragDropHandlers: Array<(event: { payload: unknown }) => void> = [];
let projectPinUpdater: ((projectId: string, pinned: boolean) => void) | null = null;
let windowScaleFactor = 1;

function cloneProject(row: ProjectRow): ProjectRow {
  return { ...row };
}

function cloneTask(row: TaskRow): TaskRow {
  return { ...row, dependsOn: [...row.dependsOn] };
}

function cloneTodo(row: TodoRow): TodoRow {
  return { ...row, attachments: [...row.attachments] };
}

function nextTodoId(): string {
  todoSeq += 1;
  return `todo-${todoSeq}`;
}

function readAgentTodoText(todo: unknown): string {
  if (typeof todo === "string") return todo.trim();
  if (!todo || typeof todo !== "object" || Array.isArray(todo)) return "";
  const row = todo as AgentTodoInput;
  const value = row.content ?? row.text ?? row.title ?? row.description;
  return typeof value === "string" ? value.trim() : "";
}

function readAgentTodoDone(todo: unknown): boolean {
  if (!todo || typeof todo !== "object" || Array.isArray(todo)) return false;
  const row = todo as AgentTodoInput;
  return row.completed === true ||
    row.done === true ||
    String(row.status ?? "").toLowerCase() === "completed";
}

function normalizeTodoPriority(value: unknown): "high" | "normal" | "low" {
  const text = typeof value === "string" ? value.trim().toLowerCase() : "";
  if (text === "high" || text === "low") return text;
  return "normal";
}

function listMockTodos(taskId: string): TodoRow[] {
  return [...(todosByTaskId[taskId] ?? [])]
    .sort((a, b) => a.order - b.order || a.createdAt - b.createdAt)
    .map(cloneTodo);
}

function setMockGuideStatus(
  guideId: unknown,
  guideStatus: "pending" | "queued" | "sent",
) {
  if (typeof guideId !== "string" || !guideId) return;
  const now = Date.now();
  for (const taskId of Object.keys(todosByTaskId)) {
    let changed = false;
    todosByTaskId[taskId] = todosByTaskId[taskId].map((todo) => {
      if (todo.id !== guideId || todo.source !== "lilia" || todo.guideStatus === guideStatus) {
        return todo;
      }
      changed = true;
      return { ...todo, guideStatus, updatedAt: now };
    });
    if (changed) {
      emitTauriEvent("todo-changed", { taskId });
      return;
    }
  }
}

function resetMockQueuedGuides(turns: Array<Record<string, unknown>> = []) {
  for (const turn of turns) {
    setMockGuideStatus(turn.guideId, "pending");
  }
}

function applyMockAgentTodos(taskId: string, todos: unknown[]): TodoRow[] {
  const now = Date.now();
  const existing = todosByTaskId[taskId] ?? [];
  const userRows = existing.filter((todo) => todo.source === "lilia");
  const agentRows = existing.filter((todo) => todo.source === "agent");
  const userMax = userRows.reduce((max, todo) => Math.max(max, todo.order), -1);
  const nextAgentRows: TodoRow[] = [];

  todos.forEach((todo, index) => {
    const text = readAgentTodoText(todo);
    if (!text) return;
    const matched = agentRows.find((row) => row.text === text);
    const nextRow: TodoRow = {
      id: matched?.id ?? nextTodoId(),
      taskId,
      text,
      done: readAgentTodoDone(todo),
      order: userMax + 1 + index,
      source: "agent",
      priority: normalizeTodoPriority((todo as AgentTodoInput)?.priority),
      guideStatus: null,
      attachments: [],
      createdAt: matched?.createdAt ?? now,
      updatedAt: now,
    };
    nextAgentRows.push(nextRow);
  });

  todosByTaskId[taskId] = [...userRows, ...nextAgentRows];
  return listMockTodos(taskId);
}

function refreshSessionCounts() {
  projects = projects.map((project) => ({
    ...project,
    sessionCount: tasks.filter((task) =>
      task.projectId === project.id && task.status !== "cancelled"
    ).length,
  }));
}

function defaultModelForBackend(backend: "claude" | "codex") {
  return backend === "codex" ? "gpt-5-codex" : "claude-sonnet-4-6";
}

function normalizeBackend(value: unknown): "claude" | "codex" {
  return value === "codex" ? "codex" : "claude";
}

function modelBelongsToBackend(model: string, backend: "claude" | "codex") {
  if (backend === "codex") {
    return ["gpt-5-codex", "o3", "o3-mini"].includes(model);
  }
  return model.startsWith("claude-");
}

function normalizeComposer(input: unknown, taskId: string) {
  const row = input && typeof input === "object" && !Array.isArray(input)
    ? input as Record<string, unknown>
    : {};
  const backend = activeBackend;
  const model = typeof row.model === "string" && modelBelongsToBackend(row.model, backend)
    ? row.model
    : defaultModelForBackend(backend);
  return {
    ...row,
    taskId,
    backend,
    model,
  };
}

export function resetTauriMockData() {
  projects = baseProjects.map(cloneProject);
  tasks = baseTasks.map(cloneTask);
  todosByTaskId = {};
  todoSeq = 0;
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
  clipboardFilePaths = [];
  clipboardImageSeq = 0;
  activeBackend = "claude";
  composerStateHandler = null;
  claudePlugins = baseClaudePlugins.map((plugin) => ({ ...plugin }));
  claudeMcpServers = baseClaudeMcpServers.map((server) => ({
    ...server,
    args: [...server.args],
    envKeys: [...server.envKeys],
  }));
  agentInteractionSettings = { nonInterruptMode: false, debug: false };
  eventHandlers = {};
  webviewDragDropHandlers = [];
  windowScaleFactor = 1;
  refreshSessionCounts();
  mockInvoke.mockClear();
  mockListen.mockClear();
  mockGetCurrentWebview.mockClear();
  mockGetCurrentWindow.mockClear();
}

export function emitTauriEvent(event: string, payload: unknown) {
  for (const handler of eventHandlers[event] ?? []) {
    handler({ payload });
  }
}

export function setMockChatRunning(taskId: string, running: boolean) {
  chatRunning[taskId] = running;
}

export function emitWebviewDragDropEvent(payload: unknown) {
  for (const handler of webviewDragDropHandlers) {
    handler({ payload });
  }
}

export function setMockComposerStateHandler(
  handler: ((taskId: string) => unknown | Promise<unknown>) | null,
) {
  composerStateHandler = handler;
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

export const mockGetCurrentWindow = vi.fn(() => ({
  isMaximized: vi.fn(async () => false),
  onResized: vi.fn(async () => vi.fn()),
  minimize: vi.fn(async () => undefined),
  toggleMaximize: vi.fn(async () => undefined),
  close: vi.fn(async () => undefined),
  scaleFactor: vi.fn(async () => windowScaleFactor),
}));

export function setMockWindowScaleFactor(scaleFactor: number) {
  windowScaleFactor = scaleFactor;
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
    setMockGuideStatus(next.guideId, "sent");
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

export function replaceMockTimelineEvents(
  taskId: string,
  events: Partial<AgentTimelineEvent>[],
) {
  timelineEvents[taskId] = events.map((patch, index) => ({
    id: patch.id ?? `tl-${taskId}-${index}`,
    taskId,
    turnId: patch.turnId ?? "turn-live",
    backend: patch.backend ?? "claude",
    kind: patch.kind ?? "command",
    status: patch.status ?? "success",
    title: patch.title ?? "历史事件",
    summary: patch.summary ?? "",
    payload: patch.payload ?? {},
    createdAt: patch.createdAt ?? 10_000 + index,
    updatedAt: patch.updatedAt ?? patch.createdAt ?? 10_000 + index,
    turnSeq: patch.turnSeq ?? index,
    intraTurnOrder: patch.intraTurnOrder ?? 0,
  }));
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

export function setMockClipboardFilePaths(paths: string[]) {
  clipboardFilePaths = [...paths];
}

export function setMockActiveBackend(backend: "claude" | "codex") {
  activeBackend = backend;
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

    case "provider_get_active_backend":
      return activeBackend;

    case "provider_set_active_backend":
      activeBackend = normalizeBackend(args.backend);
      return undefined;

    case "provider_get_config": {
      const backend = normalizeBackend(args.backend);
      return { backend, baseUrl: null, apiKey: null };
    }

    case "provider_set_config":
      return undefined;

    case "cc_switch_get_config":
      return { baseUrl: "http://127.0.0.1:15721" };

    case "cc_switch_set_config":
      return undefined;

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
      if (composerStateHandler) return composerStateHandler(taskId);
      return {
        taskId,
        backend: "claude",
        model: "claude-sonnet-4-6",
        planMode: false,
        permission: "ask",
      };
    }

    case "plugins_overview":
      return {
        claudeUserSkills: [
          {
            scope: "user",
            name: "mock-skill",
            description: "测试用 Skill",
            enabled: true,
            path: "C:\\Users\\mock\\.claude\\skills\\mock-skill\\SKILL.md",
          },
        ],
        claudeProjectSkills: [],
        claudeUserPlugins: claudePlugins.map((plugin) => ({ ...plugin })),
        claudeMcpServers: claudeMcpServers.map((server) => ({
          ...server,
          args: [...server.args],
          envKeys: [...server.envKeys],
        })),
        claudeMcpConfigPath: "C:\\Users\\mock\\.lilia\\config\\claude-mcp-servers.json",
        codexMcpServers: [
          {
            name: "mock-mcp",
            command: "node",
            args: ["mock-mcp.js"],
            enabled: true,
          },
        ],
        codexConfigPath: "C:\\Users\\mock\\.codex\\config.toml",
        warnings: [],
      };

    case "plugins_set_claude_plugin_enabled": {
      const name = String(args.name);
      const enabled = args.enabled === true;
      claudePlugins = claudePlugins.map((plugin) =>
        plugin.name === name ? { ...plugin, enabled } : plugin
      );
      return undefined;
    }

    case "plugins_create_claude_mcp_server": {
      const input = args.input as {
        name?: string;
        command?: string;
        args?: string[];
        env?: Record<string, string>;
      };
      const server = {
        name: String(input.name ?? ""),
        command: String(input.command ?? ""),
        args: Array.isArray(input.args) ? input.args.map(String) : [],
        envKeys: Object.keys(input.env ?? {}),
        enabled: true,
      };
      claudeMcpServers = [...claudeMcpServers, server];
      return { ...server, args: [...server.args], envKeys: [...server.envKeys] };
    }

    case "plugins_update_claude_mcp_server": {
      const name = String(args.name);
      const input = args.input as {
        name?: string;
        command?: string;
        args?: string[];
        env?: Record<string, string>;
        removeEnvKeys?: string[];
      };
      let updated = claudeMcpServers.find((server) => server.name === name);
      if (!updated) return undefined;
      const removed = new Set(
        Array.isArray(input.removeEnvKeys) ? input.removeEnvKeys.map(String) : [],
      );
      const envKeys = input.env
        ? [
            ...updated.envKeys.filter((key) => !removed.has(key) && !(key in input.env!)),
            ...Object.keys(input.env),
          ]
        : updated.envKeys.filter((key) => !removed.has(key));
      updated = {
        ...updated,
        name: String(input.name ?? updated.name),
        command: String(input.command ?? updated.command),
        args: Array.isArray(input.args) ? input.args.map(String) : updated.args,
        envKeys,
      };
      claudeMcpServers = claudeMcpServers.map((server) =>
        server.name === name ? updated : server
      );
      return { ...updated, args: [...updated.args], envKeys: [...updated.envKeys] };
    }

    case "plugins_delete_claude_mcp_server": {
      const name = String(args.name);
      claudeMcpServers = claudeMcpServers.filter((server) => server.name !== name);
      return undefined;
    }

    case "plugins_set_claude_mcp_server_enabled": {
      const name = String(args.name);
      const enabled = args.enabled === true;
      claudeMcpServers = claudeMcpServers.map((server) =>
        server.name === name ? { ...server, enabled } : server
      );
      return undefined;
    }

    case "plugins_open_claude_mcp_config":
      return undefined;

    case "chat_set_composer_state":
      return undefined;

    case "agent_interaction_get_settings":
      return { ...agentInteractionSettings };

    case "agent_interaction_set_settings": {
      const settings = args.settings as {
        nonInterruptMode?: unknown;
        debug?: unknown;
      } | undefined;
      agentInteractionSettings = {
        nonInterruptMode: settings?.nonInterruptMode === true,
        debug: settings?.debug === true,
      };
      return undefined;
    }

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
        const lower = path.toLowerCase();
        const exists = !lower.includes("missing") && !lower.includes("not-found");
        const isImage = /\.(png|jpe?g|gif|webp|bmp|svg)$/i.test(path);
        const isFile = path.includes(".") || isImage;
        const isBigDir = lower.includes("big-dir") || lower.includes("大型目录");
        return {
          id: `att-${index + 1}`,
          name,
          path,
          kind: exists ? (isFile ? "file" : "directory") : "unknown",
          size: exists && isFile ? 42 : null,
          exists,
          mime: exists && isImage ? "image/png" : null,
          directory: exists && !isFile
            ? {
              fileCount: isBigDir ? 250 : 12,
              directoryCount: isBigDir ? 18 : 2,
              totalSize: isBigDir ? 24 * 1024 * 1024 : 4096,
              truncated: isBigDir,
              unreadableCount: 0,
            }
            : null,
        };
      });
    }

    case "chat_read_clipboard_file_paths":
      return [...clipboardFilePaths];

    case "chat_save_clipboard_image": {
      clipboardImageSeq += 1;
      const input = args.input && typeof args.input === "object" && !Array.isArray(args.input)
        ? args.input as Record<string, unknown>
        : {};
      const mime = typeof input.mime === "string" && input.mime.startsWith("image/")
        ? input.mime
        : "image/png";
      const ext = mime === "image/jpeg"
        ? "jpg"
        : mime === "image/webp"
          ? "webp"
          : "png";
      const path = `C:\\Users\\mock\\.lilia\\cache\\clipboard-images\\clipboard-${clipboardImageSeq}.${ext}`;
      return {
        id: `clip-${clipboardImageSeq}`,
        name: `图片 ${clipboardImageSeq}.${ext}`,
        path,
        kind: "file",
        size: 42,
        exists: true,
        mime,
        directory: null,
      };
    }

    case "chat_search_context_attachments": {
      const projectCwd = String(args.projectCwd ?? "D:\\PROJECT\\workspace\\Lilia");
      const query = String(args.query ?? "").toLowerCase().replace(/\\/g, "/");
      const limit = typeof args.limit === "number" ? args.limit : 12;
      const pathMode = query.includes("/");
      const allowHidden = query.includes(".");
      const ignoredRoots = ["dist"];
      const fixtures = [
        { relativePath: "README.md", kind: "file" },
        { relativePath: "apps", kind: "directory" },
        { relativePath: "apps/desktop", kind: "directory" },
        { relativePath: "apps/desktop/src/components/chat/ChatComposer.vue", kind: "file" },
        { relativePath: "apps/desktop/src-tauri/src/lib.rs", kind: "file" },
        { relativePath: "docs/design/memory.md", kind: "file" },
        { relativePath: "screenshots/context-preview.png", kind: "file" },
        { relativePath: "big-dir", kind: "directory" },
        { relativePath: "big-dir/inside.md", kind: "file" },
        { relativePath: ".env", kind: "file" },
        { relativePath: ".github", kind: "directory" },
        { relativePath: ".github/workflows", kind: "directory" },
        { relativePath: "dist", kind: "directory" },
        { relativePath: "dist/app.js", kind: "file" },
      ];
      const directParent = (() => {
        if (!pathMode) return null;
        const normalized = query.startsWith("./") ? query.slice(2) : query;
        if (!normalized) return "";
        if (normalized.endsWith("/")) return normalized.replace(/\/+$/, "");
        return normalized.slice(0, normalized.lastIndexOf("/"));
      })();
      return fixtures
        .filter(({ relativePath }) => {
          const normalized = relativePath.toLowerCase();
          const parts = normalized.split("/");
          const name = parts.at(-1) ?? normalized;
          if (!allowHidden && parts.some((part) => part.startsWith("."))) return false;
          if (!pathMode && ignoredRoots.some((root) => normalized === root || normalized.startsWith(`${root}/`))) {
            return false;
          }
          if (directParent !== null) {
            const parent = parts.slice(0, -1).join("/");
            if (parent !== directParent) return false;
          }
          if (!query) return true;
          return normalized.includes(query) || name.includes(query);
        })
        .slice(0, limit)
        .map(({ relativePath, kind }, index) => {
          const path = `${projectCwd}\\${relativePath.replace(/\//g, "\\")}`;
          const name = relativePath.split("/").at(-1) ?? relativePath;
          const isImage = /\.(png|jpe?g|gif|webp|bmp|svg)$/i.test(relativePath);
          const isFile = kind === "file";
          const isBigDir = relativePath === "big-dir";
          return {
            attachment: {
              id: `ctx-${index + 1}`,
              name,
              path,
              kind,
              size: isFile ? 42 : null,
              exists: true,
              mime: isImage ? "image/png" : null,
              directory: !isFile
                ? {
                  fileCount: isBigDir ? 250 : 12,
                  directoryCount: isBigDir ? 18 : 2,
                  totalSize: isBigDir ? 24 * 1024 * 1024 : 4096,
                  truncated: isBigDir,
                  unreadableCount: 0,
                }
                : null,
            },
            relativePath,
            matchedBy: (name.toLowerCase().includes(query) ? "name" : "path"),
          };
        });
    }

    case "todo_list": {
      const taskId = String(args.taskId);
      return listMockTodos(taskId);
    }

    case "todo_create": {
      const taskId = String(args.taskId);
      const text = String(args.text ?? "").trim();
      const now = Date.now();
      const order = (todosByTaskId[taskId] ?? []).reduce(
        (max, todo) => Math.max(max, todo.order),
        -1,
      ) + 1;
      const todo: TodoRow = {
        id: nextTodoId(),
        taskId,
        text,
        done: false,
        order,
        source: "lilia",
        priority: normalizeTodoPriority(args.priority),
        guideStatus: "pending",
        attachments: Array.isArray(args.attachments) ? args.attachments : [],
        createdAt: now,
        updatedAt: now,
      };
      todosByTaskId[taskId] = [...(todosByTaskId[taskId] ?? []), todo];
      emitTauriEvent("todo-changed", { taskId });
      return cloneTodo(todo);
    }

    case "todo_update": {
      const id = String(args.id);
      const now = Date.now();
      for (const taskId of Object.keys(todosByTaskId)) {
        let changed = false;
        todosByTaskId[taskId] = todosByTaskId[taskId].map((todo) => {
          if (todo.id !== id) return todo;
          if (todo.source !== "lilia") return todo;
          changed = true;
          return {
            ...todo,
            text: typeof args.text === "string" ? args.text : todo.text,
            done: typeof args.done === "boolean" ? args.done : todo.done,
            order: typeof args.order === "number" ? args.order : todo.order,
            priority: args.priority === "high" || args.priority === "normal" || args.priority === "low"
              ? args.priority
              : todo.priority,
            guideStatus: args.guideStatus === "pending" ||
              args.guideStatus === "queued" ||
              args.guideStatus === "sent"
              ? args.guideStatus
              : todo.guideStatus,
            updatedAt: now,
          };
        });
        if (changed) {
          emitTauriEvent("todo-changed", { taskId });
          break;
        }
      }
      return undefined;
    }

    case "todo_delete": {
      const id = String(args.id);
      for (const taskId of Object.keys(todosByTaskId)) {
        const before = todosByTaskId[taskId].length;
        todosByTaskId[taskId] = todosByTaskId[taskId].filter((todo) =>
          todo.id !== id || todo.source !== "lilia"
        );
        if (todosByTaskId[taskId].length !== before) {
          emitTauriEvent("todo-changed", { taskId });
          break;
        }
      }
      return undefined;
    }

    case "todo_apply_agent_event": {
      const taskId = String(args.taskId);
      const todos = Array.isArray(args.todos) ? args.todos : [];
      const updated = applyMockAgentTodos(taskId, todos);
      emitTauriEvent("todo-changed", { taskId });
      return updated;
    }

    case "chat_send_message": {
      const taskId = String(args.taskId);
      const content = String(args.content);
      const composer = normalizeComposer(args.composer, taskId);
      args.composer = composer;
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
        backend: composer.backend,
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
        setMockGuideStatus(args.guideId, "queued");
        chatQueued[taskId] = [...(chatQueued[taskId] ?? []), args];
        return {
          message,
          dispatch: "queued",
          queuedCount: chatQueued[taskId].length,
        };
      }
      setMockGuideStatus(args.guideId, "sent");
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
      resetMockQueuedGuides(chatQueued[taskId]);
      chatQueued[taskId] = [];
      if (chatRunning[taskId] === true) {
        emitMockTimelineEvent(taskId, {
          id: `tl-interrupted-${turnId}`,
          backend: activeBackend,
          kind: "error",
          status: "error",
          title: "Agent 已打断",
          summary: message,
          payload: {
            backend: activeBackend,
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

    case "chat_reset_session": {
      const taskId = String(args.taskId);
      resetMockQueuedGuides(chatQueued[taskId]);
      chatQueued[taskId] = [];
      chatRunning[taskId] = false;
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
