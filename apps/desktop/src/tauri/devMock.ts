import {
  CHAT_BACKENDS,
  AGENT_TIMELINE_LIST_COMMAND,
  CHAT_ACK_RESTORED_ROLLBACK_COMMAND,
  CHAT_CHECK_ENV_COMMAND,
  CHAT_DESCRIBE_ATTACHMENTS_COMMAND,
  CHAT_GET_COMPOSER_STATE_COMMAND,
  CHAT_GET_RUNTIME_SNAPSHOT_COMMAND,
  CHAT_INTERRUPT_TURN_COMMAND,
  CHAT_READ_CLIPBOARD_FILE_PATHS_COMMAND,
  CHAT_RESPOND_AGENT_INTERACTION_COMMAND,
  CHAT_RESPOND_TITLE_UPDATE_COMMAND,
  CHAT_SEARCH_CONTEXT_ATTACHMENTS_COMMAND,
  CHAT_SEARCH_SLASH_COMMANDS_COMMAND,
  CHAT_SEND_MESSAGE_COMMAND,
  CHAT_SET_COMPOSER_STATE_COMMAND,
  DEFAULT_MODEL_BY_BACKEND,
  DEFAULT_MEMORY_SETTINGS,
  ASSISTANT_AI_GET_CONFIG_COMMAND,
  ASSISTANT_AI_OPTIMIZE_PROMPT_COMMAND,
  ASSISTANT_AI_SET_CONFIG_COMMAND,
  ASSISTANT_AI_TEST_CONNECTION_COMMAND,
  AUTOMATION_DELETE_WORKFLOW_COMMAND,
  AUTOMATION_LIST_RUNS_COMMAND,
  AUTOMATION_LIST_WORKFLOWS_COMMAND,
  AGENT_INTERACTION_DELETE_SUBAGENT_COMMAND,
  AGENT_INTERACTION_GET_SETTINGS_COMMAND,
  AGENT_INTERACTION_LIST_SUBAGENTS_COMMAND,
  AGENT_INTERACTION_SET_SETTINGS_COMMAND,
  AGENT_INTERACTION_UPSERT_SUBAGENT_COMMAND,
  CONVERSATION_SUGGESTIONS_GET_COMMAND,
  CONVERSATION_SUGGESTIONS_GET_SETTINGS_COMMAND,
  CONVERSATION_SUGGESTIONS_GET_SOURCES_COMMAND,
  CONVERSATION_SUGGESTIONS_SET_SETTINGS_COMMAND,
  GIT_CLONE_REPO_COMMAND,
  GITHUB_CLONE_REPO_COMMAND,
  GITHUB_GET_BINDING_STATUS_COMMAND,
  GITHUB_LIST_REPOS_COMMAND,
  GITHUB_POLL_DEVICE_FLOW_COMMAND,
  GITHUB_START_DEVICE_FLOW_COMMAND,
  GITHUB_UNBIND_COMMAND,
  HISTORY_IMPORT_CLEAN_BACKGROUND_TERMINALS_COMMAND,
  HISTORY_IMPORT_RUNTIME_STATES_COMMAND,
  HISTORY_IMPORT_SEARCH_COMMAND,
  MILESTONE_CREATE_COMMAND,
  MILESTONE_DELETE_COMMAND,
  MILESTONE_LIST_COMMAND,
  MILESTONE_REORDER_COMMAND,
  MILESTONE_SET_TASKS_COMMAND,
  MILESTONE_UPDATE_COMMAND,
  MEMORY_DELETE_COMMAND,
  MEMORY_GET_INJECTION_STATE_COMMAND,
  MEMORY_GET_SETTINGS_COMMAND,
  MEMORY_LIST_COMMAND,
  MEMORY_RESET_TASK_COOLDOWN_COMMAND,
  MEMORY_SET_ENABLED_COMMAND,
  MEMORY_SET_SETTINGS_COMMAND,
  MEMORY_SET_TASK_ENABLED_COMMAND,
  MEMORY_UPSERT_COMMAND,
  PROJECT_ARCHITECTURE_APPLY_COMMAND,
  PROJECT_ARCHITECTURE_GET_COMMAND,
  PROJECT_ARCHITECTURE_LIST_CHANGES_COMMAND,
  PROJECT_ARCHITECTURE_REJECT_COMMAND,
  PROJECT_ARCHITECTURE_ROLLBACK_COMMAND,
  PROJECT_CREATE_COMMAND,
  PROJECT_DASHBOARD_LIST_COMMAND,
  PROJECT_GET_COMMAND,
  PROJECT_GET_SETTINGS_COMMAND,
  PROJECT_LIST_COMMAND,
  PROJECT_REMOVE_COMMAND,
  PROJECT_RENAME_COMMAND,
  PROJECT_REORDER_COMMAND,
  PROJECT_SET_SETTINGS_COMMAND,
  PROJECT_TOGGLE_PIN_COMMAND,
  PLUGINS_CREATE_HOOK_SOURCE_COMMAND,
  PLUGINS_DELETE_HOOK_SOURCE_COMMAND,
  PLUGINS_DELETE_MCP_SERVER_COMMAND,
  PLUGINS_DELETE_SKILL_COMMAND,
  PLUGINS_HOOKS_OVERVIEW_COMMAND,
  PLUGINS_OPEN_HOOK_CONFIG_COMMAND,
  PLUGINS_OPEN_MCP_CONFIG_COMMAND,
  PLUGINS_OVERVIEW_COMMAND,
  PLUGINS_READ_HOOK_SOURCE_COMMAND,
  PLUGINS_SET_HOOK_SOURCE_ENABLED_COMMAND,
  PLUGINS_SET_MCP_SERVER_ENABLED_COMMAND,
  PLUGINS_SET_PACKAGE_ENABLED_COMMAND,
  PLUGINS_SET_SKILL_ENABLED_COMMAND,
  PLUGINS_UPDATE_HOOK_SOURCE_COMMAND,
  PROVIDER_CODEX_APP_SERVER_CHECK_UPDATE_COMMAND,
  PROVIDER_CODEX_APP_SERVER_INSTALL_UPDATE_COMMAND,
  PROVIDER_GET_ACTIVE_BACKEND_COMMAND,
  PROVIDER_GET_CONFIG_COMMAND,
  PROVIDER_SET_ACTIVE_BACKEND_COMMAND,
  PROVIDER_SET_CONFIG_COMMAND,
  POPUP_FOCUS_MAIN_COMMAND,
  POPUP_GET_WINDOW_SETTINGS_COMMAND,
  POPUP_OPEN_CHILD_QUESTION_COMMAND,
  POPUP_OPEN_NEW_CHAT_COMMAND,
  POPUP_OPEN_TASK_COMMAND,
  POPUP_REMEMBER_LAST_PROJECT_COMMAND,
  POPUP_SET_WINDOW_SETTINGS_COMMAND,
  QUOTA_USAGE_CONSUME_CODEX_RATE_LIMIT_RESET_CREDIT_COMMAND,
  QUOTA_USAGE_GET_CODEX_ACCOUNT_STATUS_COMMAND,
  QUOTA_USAGE_GET_STATS_COMMAND,
  REMOTE_CONTROL_CANCEL_PAIRING_COMMAND,
  REMOTE_CONTROL_PAIR_DEVICE_COMMAND,
  REMOTE_CONTROL_REVOKE_DEVICE_COMMAND,
  REMOTE_CONTROL_SET_HOST_ENABLED_COMMAND,
  REMOTE_CONTROL_SET_PC_NAME_COMMAND,
  REMOTE_CONTROL_START_PAIRING_COMMAND,
  REMOTE_CONTROL_STATUS_COMMAND,
  ROUTER_GET_MODE_COMMAND,
  ROUTER_SET_MODE_COMMAND,
  LILIA_IAB_OPEN_COMMAND,
  LILIA_IAB_SUBMIT_COMMAND,
  SYSTEM_OPEN_IN_VSCODE_COMMAND,
  SYSTEM_OPEN_PATH_COMMAND,
  SYSTEM_OPEN_URL_COMMAND,
  TASK_ARCHIVE_COMMAND,
  TASK_ARCHIVE_PROJECT_COMMAND,
  TASK_GET_COMMAND,
  TASK_LIST_COMMAND,
  TASK_LIST_SIDEBAR_CONVERSATIONS_COMMAND,
  TASK_PROMOTE_COMMAND,
  TASK_REORDER_COMMAND,
  TASK_REPARENT_COMMAND,
  TASK_TOGGLE_PIN_COMMAND,
  TODO_LIST_COMMAND,
  WORKTREE_ATTACH_TASK_COMMAND,
  WORKTREE_CLEANUP_ARCHIVE_COMMAND,
  WORKTREE_CLEAR_TASK_COMMAND,
  WORKTREE_CREATE_FOR_TASK_COMMAND,
  WORKTREE_GET_FOR_TASK_COMMAND,
  WORKTREE_LIST_COMMAND,
  WORKTREE_MERGE_DELETE_ARCHIVE_COMMAND,
  countProjectTaskStatuses,
  createChatBackendRecord,
  deriveProjectDashboardCounts,
  defaultRouterModeForBackend,
  createMemoryUpsertInput,
  normalizeAgentInteractionSettings,
  normalizeMemorySettings,
  normalizePermissionMode,
  routerModeUsesCodexAccount,
  type AgentInteractionSettings,
  type BackendEnvStatus,
  type ChatBackendKind,
  type Memory,
  type MemorySettings,
  type RouterMode,
} from "@lilia/contracts";
import { TAURI_PLUGIN_DIALOG_OPEN_COMMAND } from "./pluginCommands";

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

let taskWorktrees: Record<string, any> = {};

const defaultWorktreeSettings = {
  defaultMode: "current",
  parentDir: null,
  autoInstructions: [
    "This task is running inside a dedicated git worktree managed by Lilia.",
    "Keep changes scoped to this task and create commits in the worktree before requesting merge/archive.",
  ].join("\n"),
  cleanupOnArchive: true,
};

function projectDashboardRows() {
  return projects.map((project) => {
    const projectTasks = tasks.filter((task) => task.projectId === project.id);
    const statusCounts = countProjectTaskStatuses(projectTasks);
    const dashboardCounts = deriveProjectDashboardCounts(statusCounts);
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
      ...dashboardCounts,
      recentActivityAt: projectTasks.reduce<number | null>(
        (latest, task) => latest === null ? task.createdAt : Math.max(latest, task.createdAt),
        null,
      ),
      ...usage,
    };
  });
}

const emptyLists = new Set<string>([
  AGENT_TIMELINE_LIST_COMMAND,
  AUTOMATION_LIST_RUNS_COMMAND,
  AUTOMATION_LIST_WORKFLOWS_COMMAND,
  CHAT_DESCRIBE_ATTACHMENTS_COMMAND,
  CHAT_READ_CLIPBOARD_FILE_PATHS_COMMAND,
  CHAT_SEARCH_CONTEXT_ATTACHMENTS_COMMAND,
  CONVERSATION_SUGGESTIONS_GET_COMMAND,
  HISTORY_IMPORT_RUNTIME_STATES_COMMAND,
  HISTORY_IMPORT_SEARCH_COMMAND,
  PROJECT_ARCHITECTURE_LIST_CHANGES_COMMAND,
  TODO_LIST_COMMAND,
]);

const noops = new Set<string>([
  ASSISTANT_AI_SET_CONFIG_COMMAND,
  AUTOMATION_DELETE_WORKFLOW_COMMAND,
  CHAT_ACK_RESTORED_ROLLBACK_COMMAND,
  CHAT_RESPOND_AGENT_INTERACTION_COMMAND,
  CHAT_RESPOND_TITLE_UPDATE_COMMAND,
  CHAT_SET_COMPOSER_STATE_COMMAND,
  CONVERSATION_SUGGESTIONS_SET_SETTINGS_COMMAND,
  GITHUB_UNBIND_COMMAND,
  HISTORY_IMPORT_CLEAN_BACKGROUND_TERMINALS_COMMAND,
  LILIA_IAB_OPEN_COMMAND,
  MILESTONE_DELETE_COMMAND,
  MILESTONE_REORDER_COMMAND,
  MILESTONE_SET_TASKS_COMMAND,
  MILESTONE_UPDATE_COMMAND,
  PLUGINS_DELETE_MCP_SERVER_COMMAND,
  PLUGINS_DELETE_SKILL_COMMAND,
  PLUGINS_OPEN_MCP_CONFIG_COMMAND,
  PLUGINS_SET_MCP_SERVER_ENABLED_COMMAND,
  PLUGINS_SET_PACKAGE_ENABLED_COMMAND,
  PLUGINS_SET_SKILL_ENABLED_COMMAND,
  POPUP_FOCUS_MAIN_COMMAND,
  POPUP_OPEN_CHILD_QUESTION_COMMAND,
  POPUP_OPEN_NEW_CHAT_COMMAND,
  POPUP_OPEN_TASK_COMMAND,
  POPUP_REMEMBER_LAST_PROJECT_COMMAND,
  POPUP_SET_WINDOW_SETTINGS_COMMAND,
  PROJECT_REORDER_COMMAND,
  PROJECT_SET_SETTINGS_COMMAND,
  PROVIDER_SET_ACTIVE_BACKEND_COMMAND,
  PROVIDER_SET_CONFIG_COMMAND,
  ROUTER_SET_MODE_COMMAND,
  SYSTEM_OPEN_IN_VSCODE_COMMAND,
  SYSTEM_OPEN_PATH_COMMAND,
  SYSTEM_OPEN_URL_COMMAND,
  TASK_REORDER_COMMAND,
  TASK_REPARENT_COMMAND,
]);

let agentInteractionSubagents = [{
  id: "reviewer",
  name: "Reviewer",
  description: "检查风险与回归",
  instruction: "Review code changes, identify risk, and summarize findings.",
  enabled: true,
}];

let agentInteractionSettings = normalizeAgentInteractionSettings(null);
let remoteControlEnabled = false;
let remoteControlTicket: Record<string, unknown> | null = null;
let remoteControlDevices: Record<string, unknown>[] = [];
const remoteControlBridgeUrl = "http://127.0.0.1:41478";

function remoteControlStatus() {
  return {
    hostEnabled: remoteControlEnabled,
    state: remoteControlEnabled ? (remoteControlTicket ? "pairing" : "listening") : "disabled",
    pcName: "Lilia Dev PC",
    endpoint: remoteControlEnabled
      ? { endpointId: "mock-pc-endpoint", relayUrl: null, directAddresses: [] }
      : null,
    activeTicket: remoteControlTicket,
    trustedDevices: remoteControlDevices,
    capabilities: {
      protocolVersion: 1,
      minProtocolVersion: 1,
      alpn: "lilia.remote-control.v1",
      supportsPairing: true,
      supportsTaskInbox: true,
      supportsTimelineSubscription: true,
      supportsChatSend: true,
      supportsInteractionResponse: true,
      supportsInterrupt: true,
    },
  };
}

let memories: Memory[] = [
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

let memorySettings: MemorySettings = { ...DEFAULT_MEMORY_SETTINGS };

const providerBackends = CHAT_BACKENDS as readonly ChatBackendKind[];

function clone<T>(value: T): T {
  return structuredClone(value);
}

function defaultDevRouterModes(): Record<ChatBackendKind, RouterMode> {
  return createChatBackendRecord(defaultRouterModeForBackend);
}

function defaultDevBackendEnvStatus(backend: ChatBackendKind): BackendEnvStatus {
  const routerMode = defaultRouterModeForBackend(backend);
  if (routerModeUsesCodexAccount(routerMode)) {
    return {
      backend,
      hasApiKey: false,
      connectionMode: "codex-account",
      effectiveUrl: null,
    };
  }
  return {
    backend,
    hasApiKey: false,
    connectionMode: "unconfigured",
    effectiveUrl: null,
  };
}

function defaultDevBackendEnvStatuses(): Record<ChatBackendKind, BackendEnvStatus> {
  return createChatBackendRecord(defaultDevBackendEnvStatus);
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
    case TAURI_PLUGIN_DIALOG_OPEN_COMMAND:
      return null as T;
    case PROJECT_LIST_COMMAND:
      return clone(projects) as T;
    case PROJECT_DASHBOARD_LIST_COMMAND:
      return clone(projectDashboardRows()) as T;
    case PROJECT_GET_COMMAND:
      return clone(projects.find((project) => project.id === text(args, "id")) ?? null) as T;
    case PROJECT_CREATE_COMMAND:
      return clone({ ...projects[0], id: `project-${Date.now()}`, name: text(args, "name") || "未命名项目", pinned: false }) as T;
    case PROJECT_RENAME_COMMAND:
    case PROJECT_REMOVE_COMMAND:
      return true as T;
    case PROJECT_TOGGLE_PIN_COMMAND:
      return false as T;
    case PROJECT_GET_SETTINGS_COMMAND:
      return {
        cloneParentDir: "C:\\Files\\workspace",
        codexDefaults: null,
        githubBinding: null,
        worktree: defaultWorktreeSettings,
      } as T;
    case REMOTE_CONTROL_STATUS_COMMAND:
      return clone(remoteControlStatus()) as T;
    case REMOTE_CONTROL_SET_HOST_ENABLED_COMMAND:
      remoteControlEnabled = bool(args, "enabled");
      return clone(remoteControlStatus()) as T;
    case REMOTE_CONTROL_SET_PC_NAME_COMMAND:
      return clone(remoteControlStatus()) as T;
    case REMOTE_CONTROL_START_PAIRING_COMMAND:
      remoteControlEnabled = true;
      remoteControlTicket = {
        id: "mock-ticket",
        pcName: "Lilia Dev PC",
        pcEndpoint: { endpointId: "mock-pc-endpoint", relayUrl: null, directAddresses: [] },
        protocolVersion: 1,
        challenge: "mock-challenge",
        expiresAt: now + 600_000,
        bridgeUrl: remoteControlBridgeUrl,
        pairingUri: `lilia-remote://pair?v=1&ticket=mock-ticket&challenge=mock-challenge&endpoint=mock-pc-endpoint&name=Lilia%20Dev%20PC&bridge=${encodeURIComponent(remoteControlBridgeUrl)}`,
      };
      return clone(remoteControlTicket) as T;
    case REMOTE_CONTROL_CANCEL_PAIRING_COMMAND:
      remoteControlTicket = null;
      return undefined as T;
    case REMOTE_CONTROL_REVOKE_DEVICE_COMMAND:
      remoteControlDevices = remoteControlDevices.map((device) =>
        device.id === text(args, "deviceId")
          ? { ...device, trusted: false, revokedAt: now }
          : device
      );
      return clone(remoteControlStatus()) as T;
    case REMOTE_CONTROL_PAIR_DEVICE_COMMAND: {
      const input = (args.input ?? {}) as Args;
      const endpoint = (input.androidEndpoint ?? {}) as Args;
      const device = {
        id: `mock-device-${Date.now()}`,
        kind: "android",
        displayName: text(input, "deviceName") || "Android device",
        endpointId: text(endpoint, "endpointId") || "mock-android-endpoint",
        protocolVersion: 1,
        trusted: true,
        firstPairedAt: now,
        lastSeenAt: now,
        revokedAt: null,
      };
      remoteControlDevices = [device, ...remoteControlDevices];
      remoteControlTicket = null;
      return clone(device) as T;
    }
    case MEMORY_LIST_COMMAND: {
      const projectId = typeof args.projectId === "string" ? args.projectId : null;
      return clone(memories.filter((item) => item.scope === "user" || item.projectId === projectId)) as T;
    }
    case MEMORY_UPSERT_COMMAND: {
      const input = (args.input ?? {}) as Args;
      const normalized = createMemoryUpsertInput({
        id: typeof input.id === "string" ? input.id : null,
        scope: input.scope,
        projectId: text(input, "projectId") || "lilia",
        title: text(input, "title") || "新记忆",
        body: text(input, "body"),
        tags: Array.isArray(input.tags) ? input.tags : [],
        enabled: typeof input.enabled === "boolean" ? input.enabled : true,
        sourceTaskId: typeof input.sourceTaskId === "string" ? input.sourceTaskId : null,
      });
      const id = normalized.id || `memory-${Date.now()}`;
      const existing = memories.find((item) => item.id === id);
      const saved: Memory = {
        id,
        scope: normalized.scope,
        projectId: normalized.projectId ?? null,
        title: normalized.title,
        body: normalized.body,
        tags: normalized.tags ?? [],
        enabled: normalized.enabled !== false,
        sourceTaskId: normalized.sourceTaskId ?? null,
        createdAt: existing?.createdAt ?? Date.now(),
        updatedAt: Date.now(),
      };
      memories = existing
        ? memories.map((item) => item.id === id ? saved : item)
        : [saved, ...memories];
      return clone(saved) as T;
    }
    case MEMORY_SET_ENABLED_COMMAND: {
      const id = text(args, "id");
      memories = memories.map((item) =>
        item.id === id ? { ...item, enabled: bool(args, "enabled"), updatedAt: Date.now() } : item
      );
      return clone(memories.find((item) => item.id === id) ?? memories[0]) as T;
    }
    case MEMORY_DELETE_COMMAND: {
      const id = text(args, "id");
      const before = memories.length;
      memories = memories.filter((item) => item.id !== id);
      return (memories.length !== before) as T;
    }
    case MEMORY_GET_SETTINGS_COMMAND:
      return clone(memorySettings) as T;
    case MEMORY_SET_SETTINGS_COMMAND: {
      const input = (args.settings ?? {}) as Args;
      memorySettings = normalizeMemorySettings(input, memorySettings);
      return undefined as T;
    }
    case MEMORY_GET_INJECTION_STATE_COMMAND:
    case MEMORY_SET_TASK_ENABLED_COMMAND:
    case MEMORY_RESET_TASK_COOLDOWN_COMMAND:
      return {
        taskId: text(args, "taskId"),
        enabled: args.enabled !== false,
        lastInjectedTurnSeq: null,
        updatedAt: Date.now(),
      } as T;
    case TASK_LIST_COMMAND: {
      const projectId = args.projectId ?? null;
      return clone(tasks.filter((task) => task.projectId === projectId)) as T;
    }
    case TASK_LIST_SIDEBAR_CONVERSATIONS_COMMAND:
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
    case TASK_GET_COMMAND:
      return clone(tasks.find((task) => task.id === text(args, "id")) ?? null) as T;
    case TASK_PROMOTE_COMMAND:
      return clone({
        ...tasks[0],
        id: text(args, "id") || `task-${Date.now()}`,
        projectId: args.projectId ?? null,
        title: text(args, "title") || "新对话",
        createdAt: Date.now(),
      }) as T;
    case TASK_TOGGLE_PIN_COMMAND:
    case TASK_ARCHIVE_COMMAND:
      return true as T;
    case TASK_ARCHIVE_PROJECT_COMMAND:
      return 0 as T;
    case WORKTREE_LIST_COMMAND:
      return [
        {
          path: text(args, "baseRepoPath") || "C:\\Files\\workspace\\Lilia",
          head: null,
          branch: "main",
          bare: false,
          detached: false,
          prunable: false,
          locked: false,
          isMain: true,
          isTaskBound: false,
        },
        {
          path: "C:\\Files\\workspace\\Lilia-task-worktree",
          head: null,
          branch: "lilia/mock-task",
          bare: false,
          detached: false,
          prunable: false,
          locked: false,
          isMain: false,
          isTaskBound: false,
        },
      ] as T;
    case WORKTREE_GET_FOR_TASK_COMMAND:
      return clone(taskWorktrees[text(args, "taskId")] ?? null) as T;
    case WORKTREE_CLEAR_TASK_COMMAND:
      delete taskWorktrees[text(args, "taskId")];
      return undefined as T;
    case WORKTREE_CREATE_FOR_TASK_COMMAND: {
      const input = (args.input ?? {}) as Args;
      const taskId = text(input, "taskId");
      const saved = {
        taskId,
        projectId: text(input, "projectId") || null,
        baseRepoPath: text(input, "baseRepoPath"),
        worktreePath: `${text(input, "baseRepoPath") || "C:\\Files\\workspace\\Lilia"}-task-worktree`,
        branchName: `lilia/${taskId || "mock-task"}`,
        baseBranch: "main",
        status: "active",
        createdAt: Date.now(),
        updatedAt: Date.now(),
      };
      taskWorktrees[taskId] = saved;
      return clone(saved) as T;
    }
    case WORKTREE_ATTACH_TASK_COMMAND: {
      const input = (args.input ?? {}) as Args;
      const taskId = text(input, "taskId");
      const saved = {
        taskId,
        projectId: text(input, "projectId") || null,
        baseRepoPath: text(input, "baseRepoPath"),
        worktreePath: text(input, "worktreePath"),
        branchName: "lilia/mock-task",
        baseBranch: "main",
        status: "active",
        createdAt: Date.now(),
        updatedAt: Date.now(),
      };
      taskWorktrees[taskId] = saved;
      return clone(saved) as T;
    }
    case WORKTREE_CLEANUP_ARCHIVE_COMMAND: {
      const taskId = text(args, "taskId");
      delete taskWorktrees[taskId];
      return { merged: false, removed: true, archived: true, message: "mock cleaned" } as T;
    }
    case WORKTREE_MERGE_DELETE_ARCHIVE_COMMAND: {
      const taskId = text(args, "taskId");
      delete taskWorktrees[taskId];
      return { merged: true, removed: true, archived: true, message: "mock merged" } as T;
    }
    case MILESTONE_LIST_COMMAND:
      return { milestones: [], links: [] } as T;
    case MILESTONE_CREATE_COMMAND:
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
    case CHAT_CHECK_ENV_COMMAND:
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
        routerModes: defaultDevRouterModes(),
        backends: defaultDevBackendEnvStatuses(),
      } as T;
    case PROVIDER_CODEX_APP_SERVER_CHECK_UPDATE_COMMAND:
    case PROVIDER_CODEX_APP_SERVER_INSTALL_UPDATE_COMMAND:
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
    case PROVIDER_GET_ACTIVE_BACKEND_COMMAND:
      return "codex" as T;
    case PROVIDER_GET_CONFIG_COMMAND:
      return { backend: text(args, "backend") || "codex", baseUrl: null, apiKey: null, hasApiKey: false } as T;
    case ROUTER_GET_MODE_COMMAND:
      return defaultRouterModeForBackend(
        providerBackends.includes(text(args, "backend") as ChatBackendKind)
          ? text(args, "backend") as ChatBackendKind
          : "codex",
      ) as T;
    case ASSISTANT_AI_GET_CONFIG_COMMAND:
      return {
        baseUrl: null,
        apiKey: null,
        model: null,
        codexAccountSparkEnabled: false,
        hasApiKey: false,
      } as T;
    case ASSISTANT_AI_TEST_CONNECTION_COMMAND:
      return { ok: true, error: null, models: ["mock-assistant"], modelMatched: true } as T;
    case ASSISTANT_AI_OPTIMIZE_PROMPT_COMMAND:
      return [
        "请基于当前上下文处理以下任务：",
        text((args.input ?? {}) as Args, "prompt"),
        "",
        "要求：先做简单定位，明确本次修改范围，保留现有数据契约，不自动扩大任务。",
      ].join("\n") as T;
    case CONVERSATION_SUGGESTIONS_GET_SETTINGS_COMMAND:
      return { enabled: false, maxItems: 5 } as T;
    case CONVERSATION_SUGGESTIONS_GET_SOURCES_COMMAND:
      return { sources: [], localGit: null } as T;
    case CHAT_GET_COMPOSER_STATE_COMMAND:
      return {
        taskId: text(args, "taskId"),
        backend: "codex",
        model: DEFAULT_MODEL_BY_BACKEND.codex,
        planMode: false,
        goalMode: false,
        permission: normalizePermissionMode(null),
      } as T;
    case CHAT_GET_RUNTIME_SNAPSHOT_COMMAND:
      return { taskId: text(args, "taskId"), phase: "idle", backend: null, turnId: null, queuedCount: 0, pendingRollback: false, pendingResetCleanup: false, rollback: null } as T;
    case AGENT_INTERACTION_GET_SETTINGS_COMMAND:
      return clone(agentInteractionSettings) as T;
    case AGENT_INTERACTION_SET_SETTINGS_COMMAND:
      agentInteractionSettings = normalizeAgentInteractionSettings(
        (args.settings ?? null) as Partial<AgentInteractionSettings> | null,
        agentInteractionSettings,
      );
      return undefined as T;
    case AGENT_INTERACTION_LIST_SUBAGENTS_COMMAND:
      return agentInteractionSubagents.map((item) => ({ ...item })) as T;
    case AGENT_INTERACTION_UPSERT_SUBAGENT_COMMAND: {
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
    case AGENT_INTERACTION_DELETE_SUBAGENT_COMMAND:
      agentInteractionSubagents = agentInteractionSubagents.filter((item) => item.id !== text(args, "id"));
      return undefined as T;
    case CHAT_SEARCH_SLASH_COMMANDS_COMMAND:
      return [{ command: { id: "native:help", name: "help", title: "显示可用斜杠命令", description: "开发期 mock 命令。", source: "native", parameters: [] }, matchedBy: "name" }] as T;
    case CHAT_SEND_MESSAGE_COMMAND:
      return { userEvent: null } as T;
    case CHAT_INTERRUPT_TURN_COMMAND:
      return { interrupted: false, reason: "dev-mock-idle" } as T;
    case PROJECT_ARCHITECTURE_GET_COMMAND:
      return architecture(text(args, "projectId")) as T;
    case PROJECT_ARCHITECTURE_APPLY_COMMAND:
    case PROJECT_ARCHITECTURE_REJECT_COMMAND: {
      const input = (args.input ?? {}) as Args;
      return { graph: architecture(text(input, "projectId")), event: null } as T;
    }
    case PROJECT_ARCHITECTURE_ROLLBACK_COMMAND:
      return { graph: architecture(text(args, "projectId")), event: null } as T;
    case PLUGINS_OVERVIEW_COMMAND:
      return { skills: [], packages: [], mcpServers: [], configPaths: { claude: null, codex: null }, warnings: [] } as T;
    case PLUGINS_HOOKS_OVERVIEW_COMMAND:
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
    case PLUGINS_READ_HOOK_SOURCE_COMMAND:
      return {
        source: args.source,
        handlers: [],
        rawDocument: "{\n  \"hooks\": {}\n}\n",
        rawFormat: "json",
        warnings: [],
        limitations: [],
      } as T;
    case PLUGINS_UPDATE_HOOK_SOURCE_COMMAND:
      return {
        source: args.source,
        handlers: (args.input as Args)?.handlers ?? [],
        rawDocument: "{\n  \"hooks\": {}\n}\n",
        rawFormat: "json",
        warnings: [],
        limitations: [],
      } as T;
    case PLUGINS_CREATE_HOOK_SOURCE_COMMAND:
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
    case PLUGINS_DELETE_HOOK_SOURCE_COMMAND:
    case PLUGINS_SET_HOOK_SOURCE_ENABLED_COMMAND:
    case PLUGINS_OPEN_HOOK_CONFIG_COMMAND:
      return undefined as T;
    case POPUP_GET_WINDOW_SETTINGS_COMMAND:
      return { shortcut: null } as T;
    case LILIA_IAB_SUBMIT_COMMAND:
      return { submitted: false, reason: "dev-mock" } as T;
    case GITHUB_GET_BINDING_STATUS_COMMAND:
      return { state: "unbound", clientIdConfigured: false, clientIdSource: "none", binding: null } as T;
    case GITHUB_LIST_REPOS_COMMAND:
      return { items: [], nextPage: null } as T;
    case GITHUB_START_DEVICE_FLOW_COMMAND:
      return { deviceCode: "mock-device", userCode: "MOCK-DEV", verificationUri: "https://github.com/login/device", expiresAt: Date.now() + 600_000, intervalSeconds: 5 } as T;
    case GITHUB_POLL_DEVICE_FLOW_COMMAND:
      return { status: "pending", intervalSeconds: 5, bindingStatus: null, error: null } as T;
    case GIT_CLONE_REPO_COMMAND:
    case GITHUB_CLONE_REPO_COMMAND:
      return "C:\\Files\\workspace\\mock-clone" as T;
    case QUOTA_USAGE_GET_STATS_COMMAND:
      return { today: null, weekly: null, monthly: null, updatedAt: Date.now() } as T;
    case QUOTA_USAGE_GET_CODEX_ACCOUNT_STATUS_COMMAND:
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
    case QUOTA_USAGE_CONSUME_CODEX_RATE_LIMIT_RESET_CREDIT_COMMAND:
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
