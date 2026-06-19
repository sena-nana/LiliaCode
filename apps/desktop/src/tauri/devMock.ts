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
    archived: false,
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
    archived: false,
  },
];

const statusKeys = ["draft", "waiting", "running", "blocked", "done", "cancelled"] as const;

function projectDashboardRows() {
  return projects.map((project) => {
    const projectTasks = tasks.filter((task) => task.projectId === project.id);
    const statusCounts = Object.fromEntries(statusKeys.map((status) => [status, 0])) as Record<
      (typeof statusKeys)[number],
      number
    >;
    for (const task of projectTasks) {
      if (statusKeys.includes(task.status as (typeof statusKeys)[number])) {
        statusCounts[task.status as (typeof statusKeys)[number]] += 1;
      }
    }
    const usage = project.id === "lilia"
      ? { totalTokens: 12_400, knownCostUsd: 0.084, costRecordCount: 1, usageRecordCount: 2 }
      : { totalTokens: 0, knownCostUsd: null, costRecordCount: 0, usageRecordCount: 0 };
    return {
      id: project.id,
      name: project.name,
      cwd: project.cwd,
      pinned: project.pinned,
      taskCount: projectTasks.length,
      sessionCount: new Set(projectTasks.map((task) => task.sessionId)).size,
      statusCounts,
      blockedCount: statusCounts.blocked,
      activeCount: statusCounts.waiting + statusCounts.running,
      recentActivityAt: projectTasks.reduce<number | null>(
        (latest, task) => latest === null ? task.createdAt : Math.max(latest, task.createdAt),
        null,
      ),
      ...usage,
    };
  });
}

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
  "automation_delete_workflow",
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

let agentInteractionSubagents = [{
  id: "reviewer",
  name: "Reviewer",
  description: "检查风险与回归",
  instruction: "Review code changes, identify risk, and summarize findings.",
  enabled: true,
}];

type MockMemory = {
  id: string;
  scope: "user" | "project";
  projectId: string | null;
  title: string;
  body: string;
  tags: string[];
  enabled: boolean;
  sourceTaskId: string | null;
  createdAt: number;
  updatedAt: number;
};

let memories: MockMemory[] = [
  {
    id: "memory-user-1",
    scope: "user",
    projectId: null,
    title: "PR 文案",
    body: "PR 描述里不要出现 emoji。",
    tags: ["preference"],
    enabled: true,
    sourceTaskId: null,
    createdAt: now - 3_600_000,
    updatedAt: now - 3_600_000,
  },
  {
    id: "memory-project-1",
    scope: "project",
    projectId: "lilia",
    title: "迁移检查",
    body: "涉及数据库迁移时先验证 dry-run 或最小 schema 测试。",
    tags: ["database"],
    enabled: true,
    sourceTaskId: "t-001",
    createdAt: now - 1_800_000,
    updatedAt: now - 1_800_000,
  },
];

let memorySettings = {
  enabled: true,
  baselineInjectionEnabled: true,
  cooldownTurns: 5,
};

function clone<T>(value: T): T {
  return structuredClone(value);
}

function text(args: Args, key: string): string {
  return typeof args[key] === "string" ? args[key] : "";
}

function bool(args: Args, key: string, fallback = false): boolean {
  return typeof args[key] === "boolean" ? args[key] : fallback;
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
    case "project_dashboard_list":
      return clone(projectDashboardRows()) as T;
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
    case "memory_list": {
      const projectId = typeof args.projectId === "string" ? args.projectId : null;
      return clone(memories.filter((item) => item.scope === "user" || item.projectId === projectId)) as T;
    }
    case "memory_upsert": {
      const input = (args.input ?? {}) as Args;
      const scope = text(input, "scope") === "user" ? "user" : "project";
      const projectId = scope === "project" ? text(input, "projectId") || "lilia" : null;
      const id = text(input, "id") || `memory-${Date.now()}`;
      const existing = memories.find((item) => item.id === id);
      const saved: MockMemory = {
        id,
        scope,
        projectId,
        title: text(input, "title") || "新记忆",
        body: text(input, "body") || "",
        tags: Array.isArray(input.tags) ? input.tags.filter((item): item is string => typeof item === "string") : [],
        enabled: input.enabled !== false,
        sourceTaskId: typeof input.sourceTaskId === "string" ? input.sourceTaskId : null,
        createdAt: existing?.createdAt ?? Date.now(),
        updatedAt: Date.now(),
      };
      memories = existing
        ? memories.map((item) => item.id === id ? saved : item)
        : [saved, ...memories];
      return clone(saved) as T;
    }
    case "memory_set_enabled": {
      const id = text(args, "id");
      memories = memories.map((item) =>
        item.id === id ? { ...item, enabled: bool(args, "enabled"), updatedAt: Date.now() } : item
      );
      return clone(memories.find((item) => item.id === id) ?? memories[0]) as T;
    }
    case "memory_delete": {
      const id = text(args, "id");
      const before = memories.length;
      memories = memories.filter((item) => item.id !== id);
      return (memories.length !== before) as T;
    }
    case "memory_get_settings":
      return clone(memorySettings) as T;
    case "memory_set_settings": {
      const input = (args.settings ?? {}) as Args;
      const cooldownTurns =
        typeof input.cooldownTurns === "number" && Number.isFinite(input.cooldownTurns) && input.cooldownTurns > 0
          ? Math.trunc(input.cooldownTurns)
          : memorySettings.cooldownTurns;
      memorySettings = {
        enabled: typeof input.enabled === "boolean" ? input.enabled : memorySettings.enabled,
        baselineInjectionEnabled: typeof input.baselineInjectionEnabled === "boolean"
          ? input.baselineInjectionEnabled
          : memorySettings.baselineInjectionEnabled,
        cooldownTurns,
      };
      return undefined as T;
    }
    case "memory_get_injection_state":
    case "memory_set_task_enabled":
    case "memory_reset_task_cooldown":
      return {
        taskId: text(args, "taskId"),
        enabled: args.enabled !== false,
        lastInjectedTurnSeq: null,
        updatedAt: Date.now(),
      } as T;
    case "task_list": {
      const projectId = args.projectId ?? null;
      return clone(tasks.filter((task) => task.projectId === projectId)) as T;
    }
    case "task_list_sidebar_conversations":
      return clone(
        tasks
          .filter((task) => task.archived !== true)
          .sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.createdAt - a.createdAt)
          .map((task) => ({
            taskId: task.id,
            projectId: task.projectId ?? null,
            projectName: task.projectId
              ? projects.find((project) => project.id === task.projectId)?.name ?? null
              : null,
            title: task.title,
            createdAt: task.createdAt,
            pinned: task.pinned,
            route: task.projectId ? `/projects/${task.projectId}/tasks/${task.id}` : `/chats/${task.id}`,
          })),
      ) as T;
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
        codexAppServer: {
          version: "dev-mock",
          installPath: null,
          managed: false,
          available: true,
          supportsRequiredProtocol: true,
          failureKind: null,
          issues: [],
          latestVersion: null,
          updateAvailable: false,
          releaseNotes: [],
          updateError: null,
        },
        routerModes: { claude: "api", codex: "codex-account" },
        backends: {
          claude: { backend: "claude", hasApiKey: false, connectionMode: "api", effectiveUrl: null },
          codex: { backend: "codex", hasApiKey: true, connectionMode: "codex-account", effectiveUrl: null },
        },
      } as T;
    case "provider_codex_app_server_check_update":
    case "provider_codex_app_server_install_update":
      return {
        version: "dev-mock",
        installPath: null,
        managed: false,
        available: true,
        supportsRequiredProtocol: true,
        failureKind: null,
        issues: [],
        latestVersion: null,
        updateAvailable: false,
        releaseNotes: [],
        updateError: null,
      } as T;
    case "provider_get_active_backend":
      return "codex" as T;
    case "provider_get_config":
      return { backend: text(args, "backend") || "codex", baseUrl: null, apiKey: null, hasApiKey: false } as T;
    case "router_get_mode":
      return (text(args, "backend") === "codex" ? "codex-account" : "api") as T;
    case "assistant_ai_get_config":
      return {
        baseUrl: null,
        apiKey: null,
        model: null,
        codexAccountSparkEnabled: false,
        hasApiKey: false,
      } as T;
    case "assistant_ai_test_connection":
      return { ok: true, error: null, models: ["mock-assistant"], modelMatched: true } as T;
    case "assistant_ai_optimize_prompt":
      return [
        "请基于当前上下文处理以下任务：",
        text((args.input ?? {}) as Args, "prompt"),
        "",
        "要求：先做简单定位，明确本次修改范围，保留现有数据契约，不自动扩大任务。",
      ].join("\n") as T;
    case "conversation_suggestions_get_settings":
      return { enabled: false, maxItems: 5 } as T;
    case "conversation_suggestions_get_sources":
      return { sources: [], localGit: null } as T;
    case "chat_get_composer_state":
      return { taskId: text(args, "taskId"), backend: "codex", model: "gpt-5.4", planMode: false, goalMode: false, permission: "ask" } as T;
    case "chat_get_runtime_snapshot":
      return { taskId: text(args, "taskId"), phase: "idle", backend: null, turnId: null, queuedCount: 0, pendingRollback: false, pendingResetCleanup: false, rollback: null } as T;
    case "agent_interaction_get_settings":
      return {
        nonInterruptMode: false,
        debug: false,
        codexProfile: {
          profile: "default",
          model: null,
          reasoningEffort: null,
          runtimeWorkspaceRoots: [],
          responsesApiClientMetadata: null,
          additionalContext: null,
          persistExtendedHistory: null,
          initialTurnsPage: null,
          excludeTurns: [],
        },
        subagentMode: {
          enabled: false,
          codex: { enabled: true },
          claude: {
            enabled: true,
            forwardSubagentText: true,
            agentProgressSummaries: true,
          },
        },
        autoTurnDecision: {
          enabled: true,
          allowModelTier: true,
          allowReasoningEffort: true,
          allowPlanMode: true,
          allowGoalMode: true,
          allowSessionFork: true,
        },
      } as T;
    case "agent_interaction_list_subagents":
      return agentInteractionSubagents.map((item) => ({ ...item })) as T;
    case "agent_interaction_upsert_subagent": {
      const input = (args.input ?? {}) as Args;
      const saved = {
        id: text(input, "id") || `agent-${agentInteractionSubagents.length + 1}`,
        name: text(input, "name"),
        description: text(input, "description"),
        instruction: text(input, "instruction"),
        enabled: input.enabled !== false,
      };
      const index = agentInteractionSubagents.findIndex((item) => item.id === saved.id);
      if (index === -1) agentInteractionSubagents = [...agentInteractionSubagents, saved];
      else agentInteractionSubagents = agentInteractionSubagents.map((item, itemIndex) => itemIndex === index ? saved : item);
      return saved as T;
    }
    case "agent_interaction_delete_subagent":
      agentInteractionSubagents = agentInteractionSubagents.filter((item) => item.id !== text(args, "id"));
      return undefined as T;
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
    case "plugins_hooks_overview":
      return {
        sources: [
          {
            id: "dev-claude-hooks",
            backend: "claude",
            scope: "user",
            format: "claude_settings_json",
            name: "Claude User Hooks",
            path: "C:\\Users\\dev\\.claude\\settings.json",
            exists: true,
            editable: true,
            managed: false,
            enabled: true,
            handlerCount: 1,
            warnings: [],
            limitations: [],
            trustState: "unknown",
            description: "~/.claude/settings.json 中的 hooks",
          },
          {
            id: "dev-codex-hooks",
            backend: "codex",
            scope: "user",
            format: "codex_hooks_json",
            name: "Codex User Hooks",
            path: "C:\\Users\\dev\\.codex\\hooks.json",
            exists: true,
            editable: true,
            managed: false,
            enabled: true,
            handlerCount: 1,
            warnings: ["同一层同时存在 hooks.json 与 inline [hooks]；Codex 会同时加载两者。"],
            limitations: [],
            trustState: "required",
            description: "~/.codex/hooks.json",
          },
        ],
        warnings: [],
      } as T;
    case "plugins_read_hook_source":
      return {
        source: args.source,
        handlers: [],
        rawDocument: "{\n  \"hooks\": {}\n}\n",
        rawFormat: "json",
        warnings: [],
        limitations: [],
      } as T;
    case "plugins_update_hook_source":
      return {
        source: args.source,
        handlers: (args.input as Args)?.handlers ?? [],
        rawDocument: "{\n  \"hooks\": {}\n}\n",
        rawFormat: "json",
        warnings: [],
        limitations: [],
      } as T;
    case "plugins_create_hook_source":
      return {
        id: `${text(args, "backend")}-${text(args, "scope")}`,
        backend: text(args, "backend"),
        scope: text(args, "scope"),
        format: text(args, "backend") === "codex" ? "codex_hooks_json" : "claude_settings_json",
        name: "Mock Hooks",
        path: text(args, "backend") === "codex"
          ? "C:\\Users\\dev\\.codex\\hooks.json"
          : "C:\\Users\\dev\\.claude\\settings.json",
        exists: true,
        editable: true,
        managed: false,
        enabled: false,
        handlerCount: 0,
        warnings: [],
        limitations: [],
        trustState: text(args, "backend") === "codex" ? "required" : "unknown",
        description: null,
      } as T;
    case "plugins_delete_hook_source":
    case "plugins_set_hook_source_enabled":
    case "plugins_open_hook_config":
      return undefined as T;
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
      return {
        available: false,
        connectionMode: "codex-account",
        limitId: null,
        limitName: null,
        planType: null,
        rateLimitReachedType: null,
        fiveHour: null,
        weekly: null,
        sparkFiveHour: null,
        sparkWeekly: null,
        credits: null,
        sparkCredits: null,
        rateLimitResetCredits: null,
        accountUsage: null,
        usageError: null,
        fetchedAt: Date.now(),
        error: "dev-mock",
      } as T;
    case "quota_usage_consume_codex_rate_limit_reset_credit":
      return {
        outcome: "noCredit",
        status: {
          available: false,
          connectionMode: "codex-account",
          limitId: null,
          limitName: null,
          planType: null,
          rateLimitReachedType: null,
          fiveHour: null,
          weekly: null,
          sparkFiveHour: null,
          sparkWeekly: null,
          credits: null,
          sparkCredits: null,
          rateLimitResetCredits: null,
          accountUsage: null,
          usageError: null,
          fetchedAt: Date.now(),
          error: "dev-mock",
        },
      } as T;
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
