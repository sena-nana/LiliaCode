type Args = Record<string, unknown>;
type UnlistenFn = () => void;

const now = 1_720_000_000_000;

const projects = [
  {
    id: "lilia",
    name: "Lilia",
    cwd: "C:\\Files\\workspace\\Lilia",
    sessionCount: 2,
    sortOrder: 0,
    pinned: true,
  },
  {
    id: "demo",
    name: "Demo Workspace",
    cwd: "C:\\Files\\workspace\\Demo",
    sessionCount: 1,
    sortOrder: 1,
    pinned: false,
  },
];

const tasks = [
  {
    id: "t-001",
    projectId: "lilia",
    sessionId: "mock-session-001",
    title: "浏览开发期 mock 页面",
    titleSource: "manual",
    status: "running",
    createdAt: now - 86_400_000,
    parentId: null,
    dependsOn: [],
    sortOrder: 0,
    pinned: true,
  },
  {
    id: "o-001",
    projectId: null,
    sessionId: "mock-inbox-001",
    title: "收集箱 mock 对话",
    titleSource: "auto",
    status: "todo",
    createdAt: now - 21_600_000,
    parentId: null,
    dependsOn: [],
    sortOrder: 0,
    pinned: false,
  },
];

const emptyLists = new Set([
  "agent_timeline_list",
  "automation_list_runs",
  "automation_list_workflows",
  "chat_describe_attachments",
  "chat_read_clipboard_file_paths",
  "chat_search_context_attachments",
  "conversation_suggestions_get",
  "history_import_runtime_states",
  "history_import_search",
  "project_architecture_list_changes",
  "todo_list",
]);

const noops = new Set([
  "agent_interaction_set_settings",
  "assistant_ai_set_config",
  "chat_ack_restored_rollback",
  "chat_respond_agent_interaction",
  "chat_respond_title_update",
  "chat_set_composer_state",
  "conversation_suggestions_set_settings",
  "github_unbind",
  "history_import_clean_background_terminals",
  "lilia_iab_open",
  "milestone_delete",
  "milestone_reorder",
  "milestone_set_tasks",
  "milestone_update",
  "plugins_delete_mcp_server",
  "plugins_delete_skill",
  "plugins_open_mcp_config",
  "plugins_set_mcp_server_enabled",
  "plugins_set_package_enabled",
  "plugins_set_skill_enabled",
  "popup_focus_main",
  "popup_open_child_question",
  "popup_open_new_chat",
  "popup_open_task",
  "popup_remember_last_project",
  "popup_set_window_settings",
  "project_reorder",
  "project_set_settings",
  "provider_set_active_backend",
  "provider_set_config",
  "router_set_mode",
  "system_open_in_vscode",
  "system_open_path",
  "system_open_url",
  "task_reorder",
  "task_reparent",
]);

function clone<T>(value: T): T {
  return structuredClone(value);
}

function text(args: Args, key: string): string {
  return typeof args[key] === "string" ? args[key] : "";
}

function architecture(projectId: string) {
  return {
    projectId,
    version: 0,
    summary: "开发期 mock 架构图为空。",
    nodes: [],
    edges: [],
    updatedAt: now,
  };
}

export async function invoke<T>(cmd: string, args: Args = {}): Promise<T> {
  if (emptyLists.has(cmd)) return [] as T;
  if (noops.has(cmd)) return undefined as T;

  switch (cmd) {
    case "plugin:dialog|open":
      return null as T;
    case "project_list":
      return clone(projects) as T;
    case "project_get":
      return clone(projects.find((project) => project.id === text(args, "id")) ?? null) as T;
    case "project_create":
      return clone({ ...projects[0], id: `project-${Date.now()}`, name: text(args, "name") || "未命名项目", pinned: false }) as T;
    case "project_rename":
    case "project_remove":
      return true as T;
    case "project_toggle_pin":
      return false as T;
    case "project_get_settings":
      return { cloneParentDir: "C:\\Files\\workspace", codexDefaults: null, githubBinding: null } as T;
    case "task_list": {
      const projectId = args.projectId ?? null;
      return clone(tasks.filter((task) => task.projectId === projectId)) as T;
    }
    case "task_get":
      return clone(tasks.find((task) => task.id === text(args, "id")) ?? null) as T;
    case "task_promote":
      return clone({
        ...tasks[0],
        id: text(args, "id") || `task-${Date.now()}`,
        projectId: args.projectId ?? null,
        title: text(args, "title") || "新对话",
        createdAt: Date.now(),
      }) as T;
    case "task_toggle_pin":
    case "task_archive":
      return true as T;
    case "task_archive_project":
      return 0 as T;
    case "milestone_list":
      return { milestones: [], links: [] } as T;
    case "milestone_create":
      return {
        id: `milestone-${Date.now()}`,
        projectId: text(args, "projectId"),
        title: text(args, "title") || "新里程碑",
        description: "",
        status: "upcoming",
        dueDate: null,
        order: 0,
        createdAt: Date.now(),
      } as T;
    case "chat_check_env":
      return {
        nodeAvailable: true,
        codexCliAvailable: true,
        codexAppServer: { version: "dev-mock", available: true, supportsRequiredProtocol: true, failureKind: null, issues: [] },
        routerModes: { claude: "api", codex: "codex-account" },
        backends: {
          claude: { backend: "claude", hasApiKey: false, connectionMode: "api", effectiveUrl: null },
          codex: { backend: "codex", hasApiKey: true, connectionMode: "codex-account", effectiveUrl: null },
        },
      } as T;
    case "provider_get_active_backend":
      return "codex" as T;
    case "provider_get_config":
      return { backend: text(args, "backend") || "codex", baseUrl: null, apiKey: null, hasApiKey: false } as T;
    case "router_get_mode":
      return (text(args, "backend") === "codex" ? "codex-account" : "api") as T;
    case "assistant_ai_get_config":
      return { baseUrl: null, apiKey: null, model: null, hasApiKey: false } as T;
    case "assistant_ai_test_connection":
      return { ok: true, error: null, models: ["mock-assistant"], modelMatched: true } as T;
    case "conversation_suggestions_get_settings":
      return { enabled: false, maxItems: 5 } as T;
    case "chat_get_composer_state":
      return { taskId: text(args, "taskId"), backend: "codex", model: "gpt-5.4", planMode: false, permission: "ask" } as T;
    case "chat_get_runtime_snapshot":
      return { taskId: text(args, "taskId"), phase: "idle", backend: null, turnId: null, queuedCount: 0, pendingRollback: false, pendingResetCleanup: false, rollback: null } as T;
    case "chat_search_slash_commands":
      return [{ command: { id: "native:help", name: "help", title: "显示可用斜杠命令", description: "开发期 mock 命令。", source: "native", parameters: [] }, matchedBy: "name" }] as T;
    case "chat_send_message":
      return { userEvent: null } as T;
    case "chat_interrupt_turn":
      return { interrupted: false, reason: "dev-mock-idle" } as T;
    case "project_architecture_get":
      return architecture(text(args, "projectId")) as T;
    case "project_architecture_apply":
    case "project_architecture_reject": {
      const input = (args.input ?? {}) as Args;
      return { graph: architecture(text(input, "projectId")), event: null } as T;
    }
    case "project_architecture_rollback":
      return { graph: architecture(text(args, "projectId")), event: null } as T;
    case "plugins_overview":
      return { skills: [], packages: [], mcpServers: [], configPaths: { claude: null, codex: null }, warnings: [] } as T;
    case "popup_get_window_settings":
      return { shortcut: null } as T;
    case "lilia_iab_submit":
      return { submitted: false, reason: "dev-mock" } as T;
    case "github_get_binding_status":
      return { state: "unbound", clientIdConfigured: false, clientIdSource: "none", binding: null } as T;
    case "github_list_repos":
      return { items: [], nextPage: null } as T;
    case "github_start_device_flow":
      return { deviceCode: "mock-device", userCode: "MOCK-DEV", verificationUri: "https://github.com/login/device", expiresAt: Date.now() + 600_000, intervalSeconds: 5 } as T;
    case "github_poll_device_flow":
      return { status: "pending", intervalSeconds: 5, bindingStatus: null, error: null } as T;
    case "git_clone_repo":
    case "github_clone_repo":
      return "C:\\Files\\workspace\\mock-clone" as T;
    case "quota_usage_get_stats":
      return { today: null, weekly: null, monthly: null, updatedAt: Date.now() } as T;
    case "quota_usage_get_codex_account_status":
      return { available: false, error: "dev-mock" } as T;
    default:
      console.warn(`[lilia:dev-mock] Unhandled Tauri command: ${cmd}`, args);
      return undefined as T;
  }
}

export function convertFileSrc(path: string): string {
  return `asset://${path.replace(/\\/g, "/")}`;
}

export async function listen<T>(_event: string, _handler: (event: { payload: T }) => void): Promise<UnlistenFn> {
  return () => undefined;
}

export function getCurrentWindow() {
  return {
    label: "main",
    listen,
    close: async () => undefined,
    minimize: async () => undefined,
    toggleMaximize: async () => undefined,
    isMaximized: async () => false,
    setAlwaysOnTop: async () => undefined,
    setDecorations: async () => undefined,
    setIgnoreCursorEvents: async () => undefined,
    setOpacity: async () => undefined,
    setPosition: async () => undefined,
    setSize: async () => undefined,
    innerPosition: async () => ({ x: 0, y: 0 }),
    innerSize: async () => ({ width: 960, height: 720 }),
    scaleFactor: async () => 1,
    startDragging: async () => undefined,
  };
}

export function getCurrentWebview() {
  return { onDragDropEvent: listen };
}

export async function homeDir(): Promise<string> {
  return "C:\\Users\\dev";
}

export class PhysicalPosition {
  constructor(public x: number, public y: number) {}
}

export class PhysicalSize {
  constructor(public width: number, public height: number) {}
}
