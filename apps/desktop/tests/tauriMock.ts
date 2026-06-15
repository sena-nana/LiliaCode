import { vi } from "vitest";

const CODEX_MODEL_OPTIONS = [
  { id: "gpt-5.5", label: "GPT-5.5" },
  { id: "gpt-5.4", label: "GPT-5.4" },
  { id: "gpt-5.4-mini", label: "GPT-5.4 Mini" },
] as const;
const MILESTONE_STATUSES = ["upcoming", "in-progress", "done", "abandoned"] as const;
type MockMilestoneStatus = (typeof MILESTONE_STATUSES)[number];

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
  titleSource: "auto" | "manual";
  status: string;
  createdAt: number;
  parentId: string | null;
  dependsOn: string[];
  sortOrder: number;
  pinned: boolean;
  archived?: boolean;
}

interface MilestoneRow {
  id: string;
  projectId: string;
  title: string;
  description: string;
  status: MockMilestoneStatus;
  dueDate: number | null;
  order: number;
  createdAt: number;
}

interface TaskMilestoneLinkRow {
  taskId: string;
  milestoneId: string;
}

interface ProjectRoadmapRow {
  milestones: MilestoneRow[];
  links: TaskMilestoneLinkRow[];
}

interface CodexThreadRow {
  id: string;
  title: string;
  status: string | null;
  model: string | null;
  sourceKind: string | null;
  createdAt: number | null;
  updatedAt: number | null;
  archived: boolean;
  preview: string | null;
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

interface MockSlashCommand {
  id: string;
  name: string;
  title: string;
  description: string;
  source: "native" | "project";
  parameters: unknown[];
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

interface AutomationNodeRow {
  id: string;
  kind: "trigger" | "agent" | "logic" | "tool" | "human";
  title: string;
  position: { x: number; y: number };
  config: Record<string, unknown>;
}

interface AutomationEdgeRow {
  id: string;
  source: string;
  target: string;
  sourceHandle?: string | null;
  targetHandle?: string | null;
}

interface AutomationScopeRow {
  projectIds: string[];
  includeInbox: boolean;
  taskStatuses: string[];
  backends: Array<"claude" | "codex">;
  eventKinds: string[];
}

interface AutomationWorkflowRow {
  id: string;
  name: string;
  enabled: boolean;
  scope: AutomationScopeRow;
  draft: {
    nodes: AutomationNodeRow[];
    edges: AutomationEdgeRow[];
    scope: AutomationScopeRow;
  };
  publishedVersionId: string | null;
  createdAt: number;
  updatedAt: number;
}

interface AutomationRunRow {
  id: string;
  workflowId: string;
  workflowVersionId: string;
  status: "pending" | "running" | "succeeded" | "failed" | "skipped" | "waiting_user";
  trigger: {
    id: string;
    kind: string;
    projectId?: string | null;
    taskId?: string | null;
    backend?: "claude" | "codex" | null;
    eventKind?: string | null;
    automationRunId?: string | null;
    payload: Record<string, unknown>;
    createdAt: number;
  };
  scope: AutomationScopeRow;
  startedAt: number;
  finishedAt: number | null;
  error: string | null;
}

interface AutomationRunNodeStateRow {
  id: string;
  runId: string;
  nodeId: string;
  status: AutomationRunRow["status"];
  input: Record<string, unknown>;
  output: Record<string, unknown> | null;
  error: string | null;
  startedAt: number | null;
  finishedAt: number | null;
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
    titleSource: "auto",
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
    titleSource: "auto",
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
    titleSource: "auto",
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
    titleSource: "auto",
    status: "running",
    createdAt: 4000,
    parentId: null,
    dependsOn: [],
    sortOrder: 0,
    pinned: false,
  },
];

const baseMilestones: MilestoneRow[] = [
  {
    id: "m-001",
    projectId: "lilia",
    title: "首发可用路线图",
    description: "",
    status: "in-progress",
    dueDate: null,
    order: 0,
    createdAt: 5000,
  },
];

const baseTaskMilestoneLinks: TaskMilestoneLinkRow[] = [
  { taskId: "t-001", milestoneId: "m-001" },
  { taskId: "t-002", milestoneId: "m-001" },
];

const baseCodexThreads: CodexThreadRow[] = [
  {
    id: "thread-1",
    title: "打通 tsconfig paths 搜索",
    status: null,
    model: "gpt-5.5",
    sourceKind: "lilia",
    createdAt: 10_000,
    updatedAt: 20_000,
    archived: false,
    preview: "最近在检查路径别名和上下文搜索。",
  },
  {
    id: "thread-2",
    title: "整理 Codex 会话管理",
    status: "idle",
    model: "gpt-5.4",
    sourceKind: "app-server",
    createdAt: 30_000,
    updatedAt: 40_000,
    archived: false,
    preview: "讨论设置页中的会话维护入口。",
  },
  {
    id: "thread-archived",
    title: "已归档的旧会话",
    status: "completed",
    model: "gpt-5.4-mini",
    sourceKind: "app-server",
    createdAt: 5_000,
    updatedAt: 6_000,
    archived: true,
    preview: "旧的 Codex thread。",
  },
];

let projects: ProjectRow[] = [];
let tasks: TaskRow[] = [];
let milestones: MilestoneRow[] = [];
let taskMilestoneLinks: TaskMilestoneLinkRow[] = [];
let codexThreads: CodexThreadRow[] = [];
let codexTaskSessions: Record<string, string> = {};
let cleanedCodexThreads: string[] = [];
let timelineEvents: Record<string, AgentTimelineEvent[]> = {};
let todosByTaskId: Record<string, TodoRow[]> = {};
let todoSeq = 0;
let automations: AutomationWorkflowRow[] = [];
let automationRuns: AutomationRunRow[] = [];
let automationRunNodes: Record<string, AutomationRunNodeStateRow[]> = {};
let automationVersionSeq = 0;
let automationRunSeq = 0;

function defaultAutomationScope(): AutomationScopeRow {
  return {
    projectIds: [],
    includeInbox: true,
    taskStatuses: [],
    backends: [],
    eventKinds: [],
  };
}

function cloneAutomationWorkflow(row: AutomationWorkflowRow): AutomationWorkflowRow {
  return {
    ...row,
    scope: { ...row.scope, projectIds: [...row.scope.projectIds], taskStatuses: [...row.scope.taskStatuses], backends: [...row.scope.backends], eventKinds: [...row.scope.eventKinds] },
    draft: {
      nodes: row.draft.nodes.map((node) => ({
        ...node,
        position: { ...node.position },
        config: { ...node.config },
      })),
      edges: row.draft.edges.map((edge) => ({ ...edge })),
      scope: {
        ...row.draft.scope,
        projectIds: [...row.draft.scope.projectIds],
        taskStatuses: [...row.draft.scope.taskStatuses],
        backends: [...row.draft.scope.backends],
        eventKinds: [...row.draft.scope.eventKinds],
      },
    },
  };
}

function cloneAutomationRun(row: AutomationRunRow): AutomationRunRow {
  return {
    ...row,
    trigger: {
      ...row.trigger,
      payload: { ...row.trigger.payload },
    },
    scope: {
      ...row.scope,
      projectIds: [...row.scope.projectIds],
      taskStatuses: [...row.scope.taskStatuses],
      backends: [...row.scope.backends],
      eventKinds: [...row.scope.eventKinds],
    },
  };
}

function cloneAutomationRunNode(row: AutomationRunNodeStateRow): AutomationRunNodeStateRow {
  return {
    ...row,
    input: { ...row.input },
    output: row.output ? { ...row.output } : null,
  };
}
let chatRunning: Record<string, boolean> = {};
let chatQueued: Record<string, Array<Record<string, unknown>>> = {};
let runtimeSnapshotOverrides: Record<string, Record<string, unknown>> = {};
let nextChatSendError: string | null = null;
let nextAgentInteractionResponseError: string | null = null;
let clipboardFilePaths: string[] = [];
let clipboardImageSeq = 0;
let clipboardTextSeq = 0;
let liliaIabSeq = 0;
let nextLiliaIabDelivery: "runner" | "message" = "message";
let activeBackend: "claude" | "codex" = "claude";
type MockProviderBackend = "claude" | "codex";
type MockRouterMode = "api" | "codex-account";
type MockProviderConfig = {
  backend: MockProviderBackend;
  baseUrl: string | null;
  hasApiKey: boolean;
};
let providerConfigs: Record<MockProviderBackend, MockProviderConfig> = {
  claude: { backend: "claude", baseUrl: null, hasApiKey: false },
  codex: { backend: "codex", baseUrl: null, hasApiKey: false },
};
let routerModes: Record<MockProviderBackend, MockRouterMode> = {
  claude: "api",
  codex: "codex-account",
};
let nodeAvailable = true;

function createMockLiliaIabSnapshot(args: Record<string, unknown>) {
  const taskId = String(args.taskId);
  liliaIabSeq += 1;
  const path = `C:\\Users\\mock\\.lilia\\cache\\iab-snapshots\\iab-${liliaIabSeq}.png`;
  return {
    taskId,
    url: "https://example.com/debug",
    title: "Debug Page",
    note: typeof args.note === "string" && args.note.trim() ? args.note.trim() : null,
    capturedAt: Date.now(),
    screenshotPath: path,
    screenshotAttachment: {
      id: `att-iab-${liliaIabSeq}`,
      name: "IAB 截图.png",
      path,
      kind: "file",
      size: 128,
      exists: true,
      mime: "image/png",
      directory: null,
    },
    status: "captured",
    warning: null,
  };
}
let codexAppServerStatus = {
  version: "codex-cli 0.128.0",
  available: true,
  supportsRequiredProtocol: true,
  failureKind: null as
    | "missingCli"
    | "appServerUnavailable"
    | "experimentalApiUnsupported"
    | "providerIncompatible"
    | null,
  issues: [] as string[],
};
let composerStateHandler: ((taskId: string) => unknown | Promise<unknown>) | null = null;
const baseClaudePlugins = [{
  backend: "claude",
  scope: "user",
  name: "demo-plugin",
  description: "测试用 Claude plugin",
  version: "1.0.0",
  enabled: true,
  path: "C:\\Users\\mock\\.claude\\plugins\\demo-plugin",
}];
const baseClaudeMcpServers = [{
  backend: "claude",
  name: "weather",
  command: "node",
  args: ["weather-mcp.js"],
  envKeys: ["WEATHER_TOKEN"],
  enabled: true,
  editable: true,
  transport: "stdio",
}];
const baseCodexMcpServers = [
  {
    backend: "codex",
    name: "mock-mcp",
    command: "node",
    args: ["mock-mcp.js"],
    envKeys: ["MOCK_TOKEN"],
    enabled: true,
    transport: "stdio",
    editable: true,
  },
  {
    backend: "codex",
    name: "remote-mcp",
    command: "",
    args: [],
    envKeys: [],
    enabled: true,
    transport: "http",
    editable: false,
  },
];
let claudePlugins = baseClaudePlugins.map((plugin) => ({ ...plugin }));
let claudeMcpServers = baseClaudeMcpServers.map((server) => ({
  ...server,
  args: [...server.args],
  envKeys: [...server.envKeys],
}));
let codexMcpServers = baseCodexMcpServers.map((server) => ({
  ...server,
  args: [...server.args],
  envKeys: [...server.envKeys],
}));
function mcpServersForBackend(backend: string) {
  return backend === "claude" ? claudeMcpServers : codexMcpServers;
}

function updateMcpServersForBackend(
  backend: string,
  updater: (servers: typeof claudeMcpServers) => typeof claudeMcpServers,
) {
  if (backend === "claude") claudeMcpServers = updater(claudeMcpServers);
  else codexMcpServers = updater(codexMcpServers);
}

function defaultAgentInteractionSettings() {
  return {
    nonInterruptMode: false,
    debug: false,
    codexProfile: {
      profile: "default",
      model: null,
      reasoningEffort: null,
      runtimeWorkspaceRoots: [] as string[],
      responsesApiClientMetadata: null as Record<string, unknown> | null,
      additionalContext: null as string | null,
      persistExtendedHistory: null as boolean | null,
      initialTurnsPage: null as Record<string, unknown> | null,
      excludeTurns: [] as string[],
    },
  };
}

function normalizeMockJsonObject(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? { ...(value as Record<string, unknown>) }
    : null;
}

function normalizeMockStringList(value: unknown): string[] {
  return Array.isArray(value)
    ? Array.from(new Set(value.map(String).map((item) => item.trim()).filter(Boolean)))
    : [];
}

let agentInteractionSettings = defaultAgentInteractionSettings();
let assistantAIConfig = {
  baseUrl: null as string | null,
  apiKey: null as string | null,
  model: null as string | null,
  hasApiKey: true,
};
let conversationSuggestionSettings = {
  enabled: true,
  source: "assistant-ai" as "assistant-ai" | "provider",
};
let conversationSuggestionsOverride: unknown[] | null = null;
let nextConversationSuggestionsError: string | null = null;
let projectSettings = {
  cloneParentDir: null as string | null,
  codexDefaults: null as unknown,
  githubBinding: null as Record<string, unknown> | null,
};
let mockPickedFolderPath: string | null = "C:\\Users\\mock";
let githubBindingStatus = {
  state: "unbound" as "unbound" | "bound",
  clientIdConfigured: true,
  clientIdSource: "bundled" as "none" | "bundled" | "custom",
  binding: null as Record<string, unknown> | null,
};
let githubDeviceFlowStart = {
  deviceCode: "device-code-1",
  userCode: "ABCD-EFGH",
  verificationUri: "https://github.com/login/device",
  expiresAt: Date.now() + 15 * 60 * 1000,
  intervalSeconds: 1,
};
let githubDeviceFlowPollQueue: Array<Record<string, unknown>> = [];
let githubRepoPages: Record<number, { items: unknown[]; nextPage: number | null }> = {
  1: { items: [], nextPage: null },
};
let githubReposError: string | null = null;
let popupWindowSettings: { shortcut: string | null } = { shortcut: null };
let nextPopupSettingsError: string | null = null;
let popupLastProjectId: string | null = null;
let eventHandlers: Record<string, Array<(event: { payload: unknown }) => void>> = {};
let webviewDragDropHandlers: Array<(event: { payload: unknown }) => void> = [];
let projectPinUpdater: ((projectId: string, pinned: boolean) => void) | null = null;
let windowScaleFactor = 1;
let agentTimelineDelayMs = 0;
let quotaUsageStatsOverride: Record<string, unknown> | null = null;
let codexAccountQuotaStatusOverride: Record<string, unknown> | null = null;

function cloneProject(row: ProjectRow): ProjectRow {
  return { ...row };
}

function cloneTask(row: TaskRow): TaskRow {
  return { ...row, dependsOn: [...row.dependsOn] };
}

function cloneMilestone(row: MilestoneRow): MilestoneRow {
  return { ...row };
}

function cloneTaskMilestoneLink(row: TaskMilestoneLinkRow): TaskMilestoneLinkRow {
  return { ...row };
}

function dayStart(timestamp: number) {
  const dayMs = 86_400_000;
  return Math.floor(timestamp / dayMs) * dayMs;
}

function createMockQuotaUsageStats(input: Record<string, unknown> = {}) {
  const days = input.days === 30 ? 30 : 7;
  const backend = input.backend === "claude" || input.backend === "codex" ? input.backend : "all";
  const rangeEnd = dayStart(Date.now()) + 86_400_000;
  const rangeStart = rangeEnd - days * 86_400_000;
  const daily = Array.from({ length: days }, (_, index) => {
    const day = rangeStart + index * 86_400_000;
    const active = index >= days - 3;
    const inputTokens = active ? (index + 1) * 100 : 0;
    const outputTokens = active ? (index + 1) * 40 : 0;
    const cacheReadTokens = active ? index * 12 : 0;
    const cacheCreationTokens = active ? index * 5 : 0;
    return {
      dayStart: day,
      inputTokens,
      outputTokens,
      cacheReadTokens,
      cacheCreationTokens,
      totalTokens: inputTokens + outputTokens + cacheReadTokens + cacheCreationTokens,
      knownCostUsd: active && backend !== "codex" ? 0.01 * (index + 1) : null,
      costRecordCount: active && backend !== "codex" ? 1 : 0,
      recordCount: active ? 1 : 0,
    };
  });
  const totals = daily.reduce(
    (acc, row) => ({
      inputTokens: acc.inputTokens + row.inputTokens,
      outputTokens: acc.outputTokens + row.outputTokens,
      cacheReadTokens: acc.cacheReadTokens + row.cacheReadTokens,
      cacheCreationTokens: acc.cacheCreationTokens + row.cacheCreationTokens,
      totalTokens: acc.totalTokens + row.totalTokens,
    }),
    { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, totalTokens: 0 },
  );
  const backendRows = backend === "all"
    ? [
      { backend: "claude", ratio: 0.6, cost: 0.12, costRecords: 2, records: 2 },
      { backend: "codex", ratio: 0.4, cost: null, costRecords: 0, records: 1 },
    ]
    : [{ backend, ratio: 1, cost: backend === "claude" ? 0.12 : null, costRecords: backend === "claude" ? 2 : 0, records: 2 }];
  const backends = backendRows.map((row) => ({
    backend: row.backend,
    inputTokens: Math.round(totals.inputTokens * row.ratio),
    outputTokens: Math.round(totals.outputTokens * row.ratio),
    cacheReadTokens: Math.round(totals.cacheReadTokens * row.ratio),
    cacheCreationTokens: Math.round(totals.cacheCreationTokens * row.ratio),
    totalTokens: Math.round(totals.totalTokens * row.ratio),
    knownCostUsd: row.cost,
    costRecordCount: row.costRecords,
    recordCount: row.records,
  }));
  const totalRecordCount = daily.reduce((sum, row) => sum + row.recordCount, 0);
  const costRecordCount = backends.reduce((sum, row) => sum + row.costRecordCount, 0);
  const knownCostUsd = backends.reduce(
    (sum, row) => sum + (typeof row.knownCostUsd === "number" ? row.knownCostUsd : 0),
    0,
  );
  return {
    days,
    backend,
    rangeStart,
    rangeEnd,
    totals,
    cost: {
      knownCostUsd: costRecordCount > 0 ? knownCostUsd : null,
      costRecordCount,
      totalRecordCount,
    },
    daily,
    backends,
    recent: backends.map((row, index) => ({
      eventId: `usage-${row.backend}-${index}`,
      taskId: `task-${index + 1}`,
      turnId: `turn-${index + 1}`,
      backend: row.backend,
      sessionId: row.backend === "codex" ? "thread-1" : "claude-session",
      inputTokens: row.inputTokens,
      outputTokens: row.outputTokens,
      cacheReadTokens: row.cacheReadTokens,
      cacheCreationTokens: row.cacheCreationTokens,
      totalTokens: row.totalTokens,
      knownCostUsd: row.knownCostUsd,
      createdAt: rangeEnd - (index + 1) * 3_600_000,
    })),
    projects: [
      {
        projectId: "project-lilia",
        projectName: "Lilia",
        projectCwd: "C:\\Files\\workspace\\Lilia",
        ...totals,
        knownCostUsd: costRecordCount > 0 ? knownCostUsd : null,
        costRecordCount,
        recordCount: totalRecordCount,
      },
    ],
    conversations: [
      {
        taskId: "task-1",
        taskTitle: "额度统计",
        taskStatus: "running",
        projectId: "project-lilia",
        projectName: "Lilia",
        ...totals,
        knownCostUsd: costRecordCount > 0 ? knownCostUsd : null,
        costRecordCount,
        recordCount: totalRecordCount,
      },
    ],
    tools: [
      {
        key: "command::",
        label: "命令",
        kind: "command",
        subkind: null,
        toolName: null,
        callCount: 6,
        sharePercent: 60,
      },
      {
        key: "search:grep:",
        label: "内容搜索",
        kind: "search",
        subkind: "grep",
        toolName: null,
        callCount: 4,
        sharePercent: 40,
      },
    ],
  };
}

function createMockCodexAccountQuotaStatus() {
  if (routerModes.codex !== "codex-account") {
    return {
      available: false,
      connectionMode: routerModes.codex,
      limitId: null,
      limitName: null,
      planType: null,
      rateLimitReachedType: null,
      fiveHour: null,
      weekly: null,
      sparkFiveHour: null,
      sparkWeekly: null,
      fetchedAt: Date.now(),
      error: null,
    };
  }
  const nowSeconds = Math.floor(Date.now() / 1000);
  return {
    available: true,
    connectionMode: "codex-account",
    limitId: "codex",
    limitName: "Codex",
    planType: "pro",
    rateLimitReachedType: null,
    fiveHour: {
      usedPercent: 25,
      windowDurationMins: 300,
      resetsAt: nowSeconds + 3 * 60 * 60,
    },
    weekly: {
      usedPercent: 40,
      windowDurationMins: 10080,
      resetsAt: nowSeconds + 4 * 86_400,
    },
    sparkFiveHour: {
      usedPercent: 15,
      windowDurationMins: 300,
      resetsAt: nowSeconds + 2 * 60 * 60,
    },
    sparkWeekly: {
      usedPercent: 70,
      windowDurationMins: 10080,
      resetsAt: nowSeconds + 2 * 86_400,
    },
    fetchedAt: Date.now(),
    error: null,
  };
}

function getProjectRoadmap(projectId: string): ProjectRoadmapRow {
  const projectMilestones = milestones
    .filter((milestone) => milestone.projectId === projectId)
    .sort((a, b) => a.order - b.order || a.createdAt - b.createdAt)
    .map(cloneMilestone);
  const projectLinks = taskMilestoneLinks
    .filter((link) => {
      const milestone = milestones.find((item) => item.id === link.milestoneId);
      const task = tasks.find((item) => item.id === link.taskId);
      return milestone?.projectId === projectId && task?.projectId === projectId && !task.archived;
    })
    .sort((a, b) => {
      const milestoneA = milestones.find((item) => item.id === a.milestoneId);
      const milestoneB = milestones.find((item) => item.id === b.milestoneId);
      const taskA = tasks.find((item) => item.id === a.taskId);
      const taskB = tasks.find((item) => item.id === b.taskId);
      return (milestoneA?.order ?? 0) - (milestoneB?.order ?? 0) ||
        (taskA?.sortOrder ?? 0) - (taskB?.sortOrder ?? 0) ||
        (taskA?.createdAt ?? 0) - (taskB?.createdAt ?? 0);
    })
    .map(cloneTaskMilestoneLink);
  return { milestones: projectMilestones, links: projectLinks };
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
      !task.archived && task.projectId === project.id && task.status !== "cancelled"
    ).length,
  }));
}

function defaultModelForBackend(backend: "claude" | "codex") {
  return backend === "codex" ? CODEX_MODEL_OPTIONS[0].id : "claude-sonnet-4-6";
}

function normalizeBackend(value: unknown): "claude" | "codex" {
  return value === "codex" ? "codex" : "claude";
}

function directDefaultUrl(backend: MockProviderBackend) {
  return backend === "codex" ? "https://api.openai.com/v1" : "https://api.anthropic.com";
}

function cloneProviderConfig(backend: MockProviderBackend) {
  return { ...providerConfigs[backend], apiKey: null };
}

function mockBackendEnvStatus(backend: MockProviderBackend) {
  const mode = routerModes[backend];
  const config = providerConfigs[backend];
  if (mode === "codex-account" && backend === "codex") {
    return {
      backend,
      hasApiKey: false,
      connectionMode: "codex-account",
      effectiveUrl: null,
    };
  }
  const hasUrl = typeof config.baseUrl === "string" && config.baseUrl.trim().length > 0;
  const hasKey = config.hasApiKey;
  return {
    backend,
    hasApiKey: hasKey,
    connectionMode: hasKey || hasUrl ? (hasUrl ? "custom" : "api") : "unconfigured",
    effectiveUrl: hasUrl ? config.baseUrl : hasKey ? directDefaultUrl(backend) : null,
  };
}

function modelBelongsToBackend(model: string, backend: "claude" | "codex") {
  if (backend === "codex") {
    return CODEX_MODEL_OPTIONS.some((option) => option.id === model);
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

const mockSlashCommands: MockSlashCommand[] = [
  {
    id: "native:help",
    name: "help",
    title: "显示可用斜杠命令",
    description: "列出当前可执行的内置命令和项目命令。",
    source: "native",
    parameters: [],
  },
  {
    id: "native:status",
    name: "status",
    title: "显示当前会话状态",
    description: "写入当前后端和工作目录状态。",
    source: "native",
    parameters: [],
  },
  {
    id: "project:release",
    name: "release",
    title: "生成发布检查",
    description: "按项目命令模板生成发布前检查清单。",
    source: "project",
    parameters: [],
  },
];

function mockSlashCommandOutput(command: MockSlashCommand, projectCwd: string, backend: string) {
  if (command.id === "native:help") {
    return `内置命令：/help、/status\n项目命令：1 个\n项目命令文件目录：${projectCwd}\\.lilia\\commands`;
  }
  if (command.id === "native:status") {
    return `当前后端：${backend}\n工作目录：${projectCwd}`;
  }
  return "请完成发布前检查并整理风险项。\n\n来源：D:\\PROJECT\\workspace\\Lilia\\.lilia\\commands\\release.md";
}

export function resetTauriMockData() {
  projects = baseProjects.map(cloneProject);
  tasks = baseTasks.map(cloneTask);
  milestones = baseMilestones.map(cloneMilestone);
  taskMilestoneLinks = baseTaskMilestoneLinks.map(cloneTaskMilestoneLink);
  codexThreads = baseCodexThreads.map((thread) => ({ ...thread }));
  codexTaskSessions = { "t-002": "thread-1" };
  cleanedCodexThreads = [];
  todosByTaskId = {};
  todoSeq = 0;
  automationVersionSeq = 0;
  automationRunSeq = 0;
  const scope = defaultAutomationScope();
  automations = [
    {
      id: "auto-1",
      name: "任务完成后复盘",
      enabled: false,
      scope,
      draft: {
        nodes: [
          {
            id: "trigger-1",
            kind: "trigger",
            title: "任务变化",
            position: { x: 80, y: 120 },
            config: { triggerKind: "manual" },
          },
          {
            id: "agent-1",
            kind: "agent",
            title: "复盘 Agent",
            position: { x: 360, y: 120 },
            config: {
              taskId: "t-002",
              backend: "claude",
              prompt: "请复盘当前任务。",
              permission: "ask",
            },
          },
        ],
        edges: [{ id: "trigger-1-agent-1", source: "trigger-1", target: "agent-1" }],
        scope,
      },
      publishedVersionId: "auto-1-v1",
      createdAt: 1000,
      updatedAt: 1000,
    },
  ];
  automationRuns = [];
  automationRunNodes = {};
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
  runtimeSnapshotOverrides = {};
  nextChatSendError = null;
  nextAgentInteractionResponseError = null;
  clipboardFilePaths = [];
  clipboardImageSeq = 0;
  clipboardTextSeq = 0;
  liliaIabSeq = 0;
  nextLiliaIabDelivery = "message";
  activeBackend = "claude";
  providerConfigs = {
    claude: { backend: "claude", baseUrl: null, hasApiKey: false },
    codex: { backend: "codex", baseUrl: null, hasApiKey: false },
  };
  routerModes = {
    claude: "api",
    codex: "codex-account",
  };
  nodeAvailable = true;
  codexAppServerStatus = {
    version: "codex-cli 0.128.0",
    available: true,
    supportsRequiredProtocol: true,
    failureKind: null,
    issues: [],
  };
  composerStateHandler = null;
  claudePlugins = baseClaudePlugins.map((plugin) => ({ ...plugin }));
  claudeMcpServers = baseClaudeMcpServers.map((server) => ({
    ...server,
    args: [...server.args],
    envKeys: [...server.envKeys],
  }));
  codexMcpServers = baseCodexMcpServers.map((server) => ({
    ...server,
    args: [...server.args],
    envKeys: [...server.envKeys],
  }));
  agentInteractionSettings = defaultAgentInteractionSettings();
  assistantAIConfig = {
    baseUrl: null,
    apiKey: null,
    model: null,
    hasApiKey: true,
  };
  conversationSuggestionSettings = { enabled: true, source: "assistant-ai" };
  conversationSuggestionsOverride = null;
  nextConversationSuggestionsError = null;
  projectSettings = { cloneParentDir: null, codexDefaults: null, githubBinding: null };
  mockPickedFolderPath = "C:\\Users\\mock";
  githubBindingStatus = {
    state: "unbound",
    clientIdConfigured: true,
    clientIdSource: "bundled",
    binding: null,
  };
  githubDeviceFlowStart = {
    deviceCode: "device-code-1",
    userCode: "ABCD-EFGH",
    verificationUri: "https://github.com/login/device",
    expiresAt: Date.now() + 15 * 60 * 1000,
    intervalSeconds: 1,
  };
  githubDeviceFlowPollQueue = [];
  githubRepoPages = { 1: { items: [], nextPage: null } };
  githubReposError = null;
  popupWindowSettings = { shortcut: null };
  nextPopupSettingsError = null;
  popupLastProjectId = null;
  eventHandlers = {};
  webviewDragDropHandlers = [];
  windowScaleFactor = 1;
  agentTimelineDelayMs = 0;
  quotaUsageStatsOverride = null;
  codexAccountQuotaStatusOverride = null;
  mockCurrentWindow.label = "main";
  mockCurrentWindow.isMaximized.mockClear();
  mockCurrentWindow.onResized.mockClear();
  mockCurrentWindow.onMoved.mockClear();
  mockCurrentWindow.outerPosition.mockClear();
  mockCurrentWindow.innerSize.mockClear();
  mockCurrentWindow.setPosition.mockClear();
  mockCurrentWindow.setSize.mockClear();
  mockCurrentWindow.minimize.mockClear();
  mockCurrentWindow.toggleMaximize.mockClear();
  mockCurrentWindow.setAlwaysOnTop.mockClear();
  mockCurrentWindow.close.mockClear();
  mockCurrentWindow.scaleFactor.mockClear();
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

export function finishMockAutomationRun(runId: string) {
  const run = automationRuns.find((item) => item.id === runId);
  if (!run) return;
  const now = Date.now();
  run.status = "succeeded";
  run.finishedAt = now;
  run.error = null;
  automationRunNodes[runId] = (automationRunNodes[runId] ?? []).map((node) => ({
    ...node,
    status: "succeeded",
    output: { ...(node.output ?? {}), completedByEvent: true },
    finishedAt: node.finishedAt ?? now,
  }));
  emitTauriEvent("automation:run-finished", { run: cloneAutomationRun(run) });
}

export function mockListenerCount(event: string): number {
  return eventHandlers[event]?.length ?? 0;
}

export function setMockChatRunning(taskId: string, running: boolean) {
  chatRunning[taskId] = running;
}

export function setMockRuntimeSnapshot(
  taskId: string,
  snapshot: Record<string, unknown>,
) {
  runtimeSnapshotOverrides[taskId] = snapshot;
}

export function failNextMockChatSend(message: string) {
  nextChatSendError = message;
}

export function failNextMockAgentInteractionResponse(message: string) {
  nextAgentInteractionResponseError = message;
}

export function setNextMockLiliaIabDelivery(delivery: "runner" | "message") {
  nextLiliaIabDelivery = delivery;
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

export function setMockTaskArchived(taskId: string, archived: boolean) {
  tasks = tasks.map((task) =>
    task.id === taskId ? { ...task, archived } : task
  );
  refreshSessionCounts();
}

export function setMockAgentTimelineDelay(delayMs: number) {
  agentTimelineDelayMs = delayMs;
}

export function setMockConversationSuggestions(items: unknown[]) {
  conversationSuggestionsOverride = items;
}

export function failNextMockConversationSuggestions(message: string) {
  nextConversationSuggestionsError = message;
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
      .filter((task) => !task.archived && task.projectId === project.id)
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
    .filter((task) => !task.archived && task.projectId === null)
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
      parentId: task.parentId,
    }));
}

export function setMockTasks(nextTasks: TaskRow[]) {
  tasks = nextTasks.map(cloneTask);
  refreshSessionCounts();
}

export function setMockRoadmap(
  nextMilestones: MilestoneRow[],
  nextLinks: TaskMilestoneLinkRow[] = [],
) {
  milestones = nextMilestones.map(cloneMilestone);
  taskMilestoneLinks = nextLinks.map(cloneTaskMilestoneLink);
}

export const mockGetCurrentWebview = vi.fn(() => ({
  onDragDropEvent: vi.fn(async (handler: (event: { payload: unknown }) => void) => {
    webviewDragDropHandlers = [...webviewDragDropHandlers, handler];
    return async () => {
      webviewDragDropHandlers = webviewDragDropHandlers.filter((h) => h !== handler);
    };
  }),
}));

export const mockCurrentWindow = {
  label: "main",
  isMaximized: vi.fn(async () => false),
  onResized: vi.fn(async () => vi.fn()),
  onMoved: vi.fn(async () => vi.fn()),
  outerPosition: vi.fn(async () => ({ x: 80, y: 90 })),
  innerSize: vi.fn(async () => ({ width: 360, height: 520 })),
  setPosition: vi.fn(async () => undefined),
  setSize: vi.fn(async () => undefined),
  minimize: vi.fn(async () => undefined),
  toggleMaximize: vi.fn(async () => undefined),
  setAlwaysOnTop: vi.fn(async () => undefined),
  close: vi.fn(async () => undefined),
  scaleFactor: vi.fn(async () => windowScaleFactor),
};

export const mockGetCurrentWindow = vi.fn(() => mockCurrentWindow);

export function setMockCurrentWindowLabel(label: string) {
  mockCurrentWindow.label = label;
}

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

export function setMockRouterMode(
  backend: "claude" | "codex",
  mode: "api" | "codex-account" | "direct" | "cc-switch",
) {
  routerModes[backend] = backend === "codex" && mode === "codex-account"
    ? "codex-account"
    : "api";
}

export function setMockProviderConfig(
  backend: "claude" | "codex",
  config: Partial<MockProviderConfig>,
) {
  providerConfigs[backend] = {
    ...providerConfigs[backend],
    ...config,
    backend,
  };
}

export function setMockQuotaUsageStats(stats: Record<string, unknown> | null) {
  quotaUsageStatsOverride = stats ? { ...stats } : null;
}

export function setMockCodexAccountQuotaStatus(stats: Record<string, unknown> | null) {
  codexAccountQuotaStatusOverride = stats ? { ...stats } : null;
}

export function setMockNodeAvailable(available: boolean) {
  nodeAvailable = available;
}

export function setMockCodexAppServerStatus(status: Partial<typeof codexAppServerStatus>) {
  codexAppServerStatus = {
    ...codexAppServerStatus,
    ...status,
    issues: status.issues ? [...status.issues] : codexAppServerStatus.issues,
  };
}

export function failNextPopupSettingsSave(message: string) {
  nextPopupSettingsError = message;
}

export function setMockPickedFolderPath(path: string | null) {
  mockPickedFolderPath = path;
}

export function setMockGitHubBindingStatus(
  status: Partial<typeof githubBindingStatus>,
) {
  githubBindingStatus = {
    ...githubBindingStatus,
    ...status,
    binding: "binding" in status ? (status.binding ?? null) : githubBindingStatus.binding,
  };
  projectSettings = {
    ...projectSettings,
    githubBinding: githubBindingStatus.binding,
  };
}

export function setMockGitHubDeviceFlowStart(
  next: Partial<typeof githubDeviceFlowStart>,
) {
  githubDeviceFlowStart = {
    ...githubDeviceFlowStart,
    ...next,
  };
}

export function setMockGitHubPollSequence(
  queue: Array<Record<string, unknown>>,
) {
  githubDeviceFlowPollQueue = queue.map((item) => ({ ...item }));
}

export function setMockGitHubRepos(
  pages: Record<number, { items: unknown[]; nextPage: number | null }>,
) {
  githubRepoPages = Object.fromEntries(
    Object.entries(pages).map(([page, value]) => [
      Number(page),
      {
        items: Array.isArray(value.items)
          ? value.items.map((item) => ({ ...(item as Record<string, unknown>) }))
          : [],
        nextPage: value.nextPage ?? null,
      },
    ]),
  );
}

export function setMockGitHubReposError(message: string | null) {
  githubReposError = message;
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
    case "plugin:dialog|open":
      return mockPickedFolderPath;

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

    case "project_get_settings":
      return { ...projectSettings };

    case "project_set_settings": {
      const settings = args.settings && typeof args.settings === "object" && !Array.isArray(args.settings)
        ? args.settings as Record<string, unknown>
        : {};
      projectSettings = {
        cloneParentDir: typeof settings.cloneParentDir === "string"
          ? settings.cloneParentDir
          : null,
        codexDefaults: "codexDefaults" in settings
          ? settings.codexDefaults
          : null,
        githubBinding: "githubBinding" in settings
          ? settings.githubBinding as Record<string, unknown> | null
          : null,
      };
      return undefined;
    }

    case "git_clone_repo": {
      const url = String(args.url ?? "").trim().replace(/\.git$/i, "").replace(/\/+$/, "");
      const parentDir = String(args.parentDir ?? "").replace(/[\\/]+$/, "");
      const base = url.split(/[/:]/).pop() || "repo";
      return `${parentDir}\\${base}`;
    }

    case "github_get_binding_status":
      return {
        ...githubBindingStatus,
        binding: githubBindingStatus.binding ? { ...githubBindingStatus.binding } : null,
      };

    case "github_start_device_flow":
      return { ...githubDeviceFlowStart };

    case "github_poll_device_flow": {
      const next = githubDeviceFlowPollQueue.shift() ?? {
        status: "pending",
        intervalSeconds: githubDeviceFlowStart.intervalSeconds,
        bindingStatus: null,
        error: null,
      };
      const result = {
        status: String(next.status ?? "pending"),
        intervalSeconds: Number(next.intervalSeconds ?? githubDeviceFlowStart.intervalSeconds),
        bindingStatus: next.bindingStatus ?? null,
        error: next.error ?? null,
      };
      if (result.status === "authorized" && result.bindingStatus && typeof result.bindingStatus === "object") {
        const bindingStatus = result.bindingStatus as Record<string, unknown>;
        githubBindingStatus = {
          state: bindingStatus.state === "bound" ? "bound" : "unbound",
          clientIdConfigured: bindingStatus.clientIdConfigured !== false,
          clientIdSource: bindingStatus.clientIdSource === "custom"
            ? "custom"
            : bindingStatus.clientIdSource === "none"
              ? "none"
              : "bundled",
          binding: bindingStatus.binding && typeof bindingStatus.binding === "object"
            ? bindingStatus.binding as Record<string, unknown>
            : null,
        };
        projectSettings = {
          ...projectSettings,
          githubBinding: githubBindingStatus.binding,
        };
      }
      return result;
    }

    case "github_unbind":
      githubBindingStatus = {
        ...githubBindingStatus,
        state: "unbound",
        binding: null,
      };
      projectSettings = { ...projectSettings, githubBinding: null };
      return undefined;

    case "github_list_repos": {
      if (githubReposError) {
        throw new Error(githubReposError);
      }
      const page = typeof args.page === "number" ? args.page : 1;
      const result = githubRepoPages[page] ?? { items: [], nextPage: null };
      return {
        items: result.items.map((item) => ({ ...(item as Record<string, unknown>) })),
        nextPage: result.nextPage,
      };
    }

    case "github_clone_repo": {
      const repo = String(args.repo ?? "").trim().replace(/\/+$/, "");
      const parentDir = String(args.parentDir ?? "").replace(/[\\/]+$/, "");
      const cleaned = repo.replace(/\.git$/i, "");
      const base = cleaned.split(/[/:]/).pop() || "repo";
      return `${parentDir}\\${base}`;
    }

    case "system_open_path":
    case "system_open_url":
    case "system_open_in_vscode":
      return undefined;

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
      const removedMilestoneIds = new Set(
        milestones
          .filter((milestone) => milestone.projectId === id)
          .map((milestone) => milestone.id),
      );
      milestones = milestones.filter((milestone) => milestone.projectId !== id);
      taskMilestoneLinks = taskMilestoneLinks.filter((link) =>
        !removedMilestoneIds.has(link.milestoneId)
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

    case "milestone_list": {
      return getProjectRoadmap(String(args.projectId));
    }

    case "milestone_create": {
      const projectId = String(args.projectId);
      const title = String(args.title ?? "").trim();
      if (!title) throw new Error("milestone_create: 标题不能为空");
      if (!projects.some((project) => project.id === projectId)) {
        throw new Error("milestone_create: 项目不存在");
      }
      const row: MilestoneRow = {
        id: `m-${milestones.length + 1}`,
        projectId,
        title,
        description: "",
        status: "upcoming",
        dueDate: null,
        order: milestones.filter((milestone) => milestone.projectId === projectId).length,
        createdAt: Date.now(),
      };
      milestones = [...milestones, row];
      return cloneMilestone(row);
    }

    case "milestone_update": {
      const id = String(args.id);
      const title = typeof args.title === "string" ? args.title.trim() : null;
      const description = typeof args.description === "string" ? args.description.trim() : null;
      const status = typeof args.status === "string" ? args.status : null;
      const dueDate = typeof args.dueDate === "number"
        ? args.dueDate
        : args.clearDueDate === true
          ? null
          : undefined;
      if (title !== null && !title) throw new Error("milestone_update: 标题不能为空");
      if (status !== null && !MILESTONE_STATUSES.includes(status as MockMilestoneStatus)) {
        throw new Error(`milestone_update: 无效状态：${status}`);
      }
      let changed = false;
      milestones = milestones.map((milestone) => {
        if (milestone.id !== id) return milestone;
        changed = true;
        return {
          ...milestone,
          title: title ?? milestone.title,
          description: description ?? milestone.description,
          status: (status ?? milestone.status) as MockMilestoneStatus,
          dueDate: dueDate === undefined ? milestone.dueDate : dueDate,
        };
      });
      if (!changed) throw new Error("milestone_update: milestone 不存在");
      return undefined;
    }

    case "milestone_set_tasks": {
      const milestoneId = String(args.milestoneId);
      const taskIds = Array.isArray(args.taskIds) ? Array.from(new Set(args.taskIds.map(String))) : [];
      const milestone = milestones.find((item) => item.id === milestoneId);
      if (!milestone) throw new Error("milestone_set_tasks: milestone 不存在");
      for (const taskId of taskIds) {
        const task = tasks.find((item) => item.id === taskId);
        if (!task || task.archived || task.projectId !== milestone.projectId) {
          throw new Error(`milestone_set_tasks: 任务不属于当前项目：${taskId}`);
        }
      }
      taskMilestoneLinks = [
        ...taskMilestoneLinks.filter((link) => link.milestoneId !== milestoneId),
        ...taskIds.map((taskId) => ({ taskId, milestoneId })),
      ];
      return taskIds.map((taskId) => ({ taskId, milestoneId }));
    }

    case "popup_get_window_settings":
      return { ...popupWindowSettings };

    case "popup_set_window_settings": {
      if (nextPopupSettingsError) {
        const message = nextPopupSettingsError;
        nextPopupSettingsError = null;
        throw new Error(message);
      }
      const input = args.settings && typeof args.settings === "object" && !Array.isArray(args.settings)
        ? args.settings as Record<string, unknown>
        : {};
      const shortcut = typeof input.shortcut === "string" && input.shortcut.trim()
        ? input.shortcut.trim()
        : null;
      popupWindowSettings = { shortcut };
      return undefined;
    }

    case "popup_remember_last_project": {
      const projectId = String(args.projectId ?? "");
      if (projects.some((project) => project.id === projectId)) {
        popupLastProjectId = projectId;
      }
      return undefined;
    }

    case "popup_open_new_chat": {
      const projectId = typeof args.projectId === "string" ? args.projectId : null;
      if (projectId) popupLastProjectId = projectId;
      return undefined;
    }

    case "popup_open_task": {
      const projectId = typeof args.projectId === "string" ? args.projectId : null;
      if (projectId) popupLastProjectId = projectId;
      return undefined;
    }

    case "popup_open_child_question": {
      const projectId = typeof args.projectId === "string" ? args.projectId : null;
      if (projectId) popupLastProjectId = projectId;
      return undefined;
    }

    case "popup_focus_main":
      emitTauriEvent("lilia:main:navigate", { route: String(args.route ?? "/") });
      return undefined;

    case "task_list": {
      const projectId = args.projectId as string | null | undefined;
      return tasks
        .filter((task) => !task.archived && task.projectId === (projectId ?? null))
        .map(cloneTask)
        .sort((a, b) =>
          Number(b.pinned) - Number(a.pinned) || a.sortOrder - b.sortOrder
        );
    }

    case "task_get": {
      const id = String(args.id);
      const row = tasks.find((task) => !task.archived && task.id === id);
      return row ? cloneTask(row) : null;
    }

    case "task_update": {
      const id = String(args.id);
      const title = typeof args.title === "string" ? args.title : null;
      const status = typeof args.status === "string" ? args.status : null;
      let changedProjectId: string | null = null;
      let changed = false;
      tasks = tasks.map((task) => {
        if (task.id !== id || task.archived) return task;
        changedProjectId = task.projectId;
        changed = true;
        return {
          ...task,
          title: title ?? task.title,
          titleSource: title === null ? task.titleSource : "manual",
          status: status ?? task.status,
        };
      });
      if (changed) emitTauriEvent("tasks:changed", { projectId: changedProjectId });
      return undefined;
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

    case "task_create": {
      const projectId = typeof args.projectId === "string" ? args.projectId : null;
      const title = String(args.title ?? "新对话");
      const status = String(args.status ?? "draft");
      const dependsOn = Array.isArray(args.dependsOn) ? args.dependsOn.map(String) : [];
      const id = `mock-task-${tasks.length + 1}`;
      const row: TaskRow = {
        id,
        projectId,
        sessionId: id,
        title,
        titleSource: "auto",
        status,
        createdAt: Date.now(),
        parentId: typeof args.parentId === "string" ? args.parentId : null,
        dependsOn,
        sortOrder: tasks.filter((task) => task.projectId === projectId).length,
        pinned: false,
      };
      tasks = [row, ...tasks];
      refreshSessionCounts();
      emitTauriEvent("tasks:changed", { projectId });
      return cloneTask(row);
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
        titleSource: "auto",
        status: "running",
        createdAt: Date.now(),
        parentId: typeof args.parentId === "string" ? args.parentId : null,
        dependsOn,
        sortOrder,
        pinned: false,
      };
      tasks = [row, ...tasks.filter((task) => task.id !== id)];
      refreshSessionCounts();
      emitTauriEvent("tasks:changed", { projectId });
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

    case "automation_list_workflows":
      return automations.map(cloneAutomationWorkflow);

    case "automation_save_draft": {
      const input = args.input && typeof args.input === "object" && !Array.isArray(args.input)
        ? args.input as Record<string, unknown>
        : {};
      const id = typeof input.id === "string" && input.id ? input.id : `auto-${automations.length + 1}`;
      const now = Date.now();
      const scopeInput = input.scope && typeof input.scope === "object" && !Array.isArray(input.scope)
        ? input.scope as Partial<AutomationScopeRow>
        : {};
      const nextScope: AutomationScopeRow = {
        ...defaultAutomationScope(),
        projectIds: Array.isArray(scopeInput.projectIds) ? scopeInput.projectIds.map(String) : [],
        includeInbox: scopeInput.includeInbox !== false,
        taskStatuses: Array.isArray(scopeInput.taskStatuses) ? scopeInput.taskStatuses.map(String) : [],
        backends: Array.isArray(scopeInput.backends)
          ? scopeInput.backends.filter((item): item is "claude" | "codex" => item === "claude" || item === "codex")
          : [],
        eventKinds: Array.isArray(scopeInput.eventKinds) ? scopeInput.eventKinds.map(String) : [],
      };
      const nodes = Array.isArray(input.nodes)
        ? input.nodes.map((node, index) => {
          const row = node && typeof node === "object" && !Array.isArray(node)
            ? node as Record<string, unknown>
            : {};
          return {
            id: typeof row.id === "string" ? row.id : `node-${index}`,
            kind: row.kind === "trigger" || row.kind === "agent" || row.kind === "logic" || row.kind === "tool" || row.kind === "human"
              ? row.kind
              : "tool",
            title: typeof row.title === "string" ? row.title : "节点",
            position: row.position && typeof row.position === "object" && !Array.isArray(row.position)
              ? {
                x: Number((row.position as Record<string, unknown>).x ?? 0),
                y: Number((row.position as Record<string, unknown>).y ?? 0),
              }
              : { x: 0, y: 0 },
            config: row.config && typeof row.config === "object" && !Array.isArray(row.config)
              ? { ...row.config as Record<string, unknown> }
              : {},
          } satisfies AutomationNodeRow;
        })
        : [];
      const edges = Array.isArray(input.edges)
        ? input.edges.map((edge, index) => {
          const row = edge && typeof edge === "object" && !Array.isArray(edge)
            ? edge as Record<string, unknown>
            : {};
          return {
            id: typeof row.id === "string" ? row.id : `edge-${index}`,
            source: String(row.source ?? ""),
            target: String(row.target ?? ""),
            sourceHandle: typeof row.sourceHandle === "string" ? row.sourceHandle : null,
            targetHandle: typeof row.targetHandle === "string" ? row.targetHandle : null,
          } satisfies AutomationEdgeRow;
        })
        : [];
      const existing = automations.find((item) => item.id === id);
      const workflow: AutomationWorkflowRow = {
        id,
        name: String(input.name ?? "新自动化"),
        enabled: existing?.enabled ?? false,
        scope: nextScope,
        draft: { nodes, edges, scope: nextScope },
        publishedVersionId: existing?.publishedVersionId ?? null,
        createdAt: existing?.createdAt ?? now,
        updatedAt: now,
      };
      automations = [workflow, ...automations.filter((item) => item.id !== id)];
      emitTauriEvent("automation:changed", { workflowId: id });
      return cloneAutomationWorkflow(workflow);
    }

    case "automation_publish": {
      const id = String(args.id);
      const workflow = automations.find((item) => item.id === id);
      if (!workflow) throw new Error("自动化不存在");
      automationVersionSeq += 1;
      const versionId = `${id}-v${automationVersionSeq + 1}`;
      workflow.publishedVersionId = versionId;
      workflow.updatedAt = Date.now();
      emitTauriEvent("automation:changed", { workflowId: id });
      return {
        id: versionId,
        workflowId: id,
        version: automationVersionSeq + 1,
        snapshot: workflow.draft,
        createdAt: Date.now(),
      };
    }

    case "automation_set_enabled": {
      const id = String(args.id);
      const enabled = args.enabled === true;
      automations = automations.map((workflow) =>
        workflow.id === id ? { ...workflow, enabled, updatedAt: Date.now() } : workflow
      );
      emitTauriEvent("automation:changed", { workflowId: id });
      return undefined;
    }

    case "automation_run_once": {
      const id = String(args.id);
      const workflow = automations.find((item) => item.id === id);
      if (!workflow?.publishedVersionId) throw new Error("运行前需要先发布");
      automationRunSeq += 1;
      const now = Date.now();
      const waitingNode = workflow.draft.nodes.find((node) => node.kind === "human");
      const waitingAgent = waitingNode ? null : workflow.draft.nodes.find((node) => node.kind === "agent");
      const run: AutomationRunRow = {
        id: `run-${automationRunSeq}`,
        workflowId: id,
        workflowVersionId: workflow.publishedVersionId,
        status: waitingNode ? "waiting_user" : waitingAgent ? "running" : "succeeded",
        trigger: {
          id: `signal-${automationRunSeq}`,
          kind: "manual",
          projectId: workflow.scope.projectIds[0] ?? null,
          taskId: "t-002",
          backend: workflow.scope.backends[0] ?? "claude",
          eventKind: "manual",
          automationRunId: null,
          payload: {},
          createdAt: now,
        },
        scope: workflow.scope,
        startedAt: now,
        finishedAt: waitingNode || waitingAgent ? null : now,
        error: null,
      };
      automationRuns = [run, ...automationRuns];
      automationRunNodes[run.id] = workflow.draft.nodes.map((node) => ({
        id: `${run.id}:${node.id}`,
        runId: run.id,
        nodeId: node.id,
        status: node.id === waitingNode?.id
          ? "waiting_user"
          : node.id === waitingAgent?.id
            ? "running"
            : "succeeded",
        input: { trigger: run.trigger },
        output: node.id === waitingNode?.id
          ? { waitingUser: true, prompt: String(node.config.prompt ?? "确认后继续执行自动化。") }
          : node.id === waitingAgent?.id
            ? { waitingAgent: true, taskId: "t-002", turnId: `turn-${run.id}` }
          : { ok: true },
        error: null,
        startedAt: now,
        finishedAt: node.id === waitingNode?.id || node.id === waitingAgent?.id ? null : now,
      }));
      emitTauriEvent("automation:run-started", { run: cloneAutomationRun(run) });
      emitTauriEvent(waitingNode || waitingAgent ? "automation:run-updated" : "automation:run-finished", { run: cloneAutomationRun(run) });
      return cloneAutomationRun(run);
    }

    case "automation_resume_run": {
      const runId = String(args.runId);
      const run = automationRuns.find((item) => item.id === runId);
      if (!run) throw new Error("运行不存在");
      const now = Date.now();
      run.status = "succeeded";
      run.finishedAt = now;
      automationRunNodes[runId] = (automationRunNodes[runId] ?? []).map((node) =>
        node.status === "waiting_user"
          ? {
            ...node,
            status: "succeeded",
            output: { waitingUser: false, confirmed: true },
            finishedAt: now,
          }
          : node,
      );
      emitTauriEvent("automation:run-finished", { run: cloneAutomationRun(run) });
      return cloneAutomationRun(run);
    }

    case "automation_list_runs": {
      const workflowId = typeof args.workflowId === "string" ? args.workflowId : null;
      return automationRuns
        .filter((run) => !workflowId || run.workflowId === workflowId)
        .map(cloneAutomationRun);
    }

    case "automation_get_run": {
      const runId = String(args.runId);
      const run = automationRuns.find((item) => item.id === runId);
      if (!run) return null;
      return {
        run: cloneAutomationRun(run),
        nodes: (automationRunNodes[runId] ?? []).map(cloneAutomationRunNode),
      };
    }

    case "chat_check_env":
      return {
        nodeAvailable,
        codexCliAvailable: codexAppServerStatus.failureKind !== "missingCli",
        codexAppServer: {
          ...codexAppServerStatus,
          issues: [...codexAppServerStatus.issues],
        },
        routerModes: {
          ...routerModes,
        },
        backends: {
          claude: mockBackendEnvStatus("claude"),
          codex: mockBackendEnvStatus("codex"),
        },
      };

    case "provider_get_active_backend":
      return activeBackend;

    case "provider_set_active_backend":
      activeBackend = normalizeBackend(args.backend);
      return undefined;

    case "provider_get_config": {
      const backend = normalizeBackend(args.backend);
      return cloneProviderConfig(backend);
    }

    case "provider_set_config": {
      const input = args.config && typeof args.config === "object" && !Array.isArray(args.config)
        ? args.config as Record<string, unknown>
        : {};
      const backend = normalizeBackend(input.backend);
      providerConfigs[backend] = {
        backend,
        baseUrl: typeof input.baseUrl === "string" && input.baseUrl.trim()
          ? input.baseUrl.trim()
          : null,
        apiKey: null,
        hasApiKey: input.clearApiKey === true
          ? false
          : typeof input.apiKey === "string" && input.apiKey.trim()
            ? true
            : providerConfigs[backend].hasApiKey,
      };
      return undefined;
    }

    case "assistant_ai_get_config":
      return { ...assistantAIConfig, apiKey: null };

    case "assistant_ai_set_config": {
      const config = args.config && typeof args.config === "object" && !Array.isArray(args.config)
        ? args.config as Record<string, unknown>
        : {};
      assistantAIConfig = {
        baseUrl: typeof config.baseUrl === "string" && config.baseUrl.trim() ? config.baseUrl.trim() : null,
        apiKey: null,
        model: typeof config.model === "string" && config.model.trim() ? config.model.trim() : null,
        hasApiKey: config.clearApiKey === true
          ? false
          : typeof config.apiKey === "string" && config.apiKey.trim()
            ? true
            : assistantAIConfig.hasApiKey,
      };
      return undefined;
    }

    case "assistant_ai_test_connection":
      return { ok: false, error: "baseUrl / apiKey / model 必须全部填写", models: null, modelMatched: null };

    case "conversation_suggestions_get_settings":
      return { ...conversationSuggestionSettings };

    case "conversation_suggestions_set_settings": {
      const settings = args.settings && typeof args.settings === "object" && !Array.isArray(args.settings)
        ? args.settings as Record<string, unknown>
        : {};
      conversationSuggestionSettings = {
        enabled: settings.enabled !== false,
        source: settings.source === "provider" ? "provider" : "assistant-ai",
      };
      return undefined;
    }

    case "conversation_suggestions_get":
      if (!conversationSuggestionSettings.enabled) return [];
      if (nextConversationSuggestionsError) {
        const message = nextConversationSuggestionsError;
        nextConversationSuggestionsError = null;
        throw new Error(message);
      }
      if (conversationSuggestionsOverride) return conversationSuggestionsOverride;
      return [
        {
          id: "sg-1",
          projectId: typeof args.projectId === "string" ? args.projectId : null,
          taskIds: ["t-002"],
          source: "task",
          githubActivities: [],
          summary: "补齐建议缓存测试",
          reason: "最近对话已经接入了设置与空草稿入口，但缓存失效路径还需要验证。",
          prompt: "请检查新对话建议的缓存命中、强制刷新和最近 timeline 更新失效逻辑，并补齐最小测试。",
          generatedAt: Date.now(),
        },
        {
          id: "sg-2",
          projectId: typeof args.projectId === "string" ? args.projectId : null,
          taskIds: ["t-002"],
          source: "github",
          githubActivities: [
            {
              id: "gh-1",
              repoFullName: "sena-nana/LiliaCode",
              kind: "pull_request",
              title: "PR #1: 优化空状态体验",
              url: "https://github.com/sena-nana/LiliaCode/pull/1",
            },
          ],
          summary: "优化空状态体验",
          reason: "空草稿页现在可以显示建议，下一步可以确认加载失败和无建议时的轻量反馈。",
          prompt: "请优化空白新对话页的建议加载失败和无建议状态，保持不阻塞输入并符合现有样式。",
          generatedAt: Date.now(),
        },
      ];

    case "history_import_search": {
      const input = args.input && typeof args.input === "object" && !Array.isArray(args.input)
        ? args.input as Record<string, unknown>
        : {};
      const provider = input.provider === "claude" ? "claude" : "codex";
      if (provider === "claude") {
        return {
          items: [],
          nextCursor: null,
        };
      }
      const term = String(input.searchTerm ?? "").trim().toLowerCase();
      const includeArchived = input.archived === true;
      const limit = typeof input.limit === "number" ? Math.max(1, input.limit) : 20;
      const cursor = typeof input.cursor === "string" && input.cursor.startsWith("offset:")
        ? Number(input.cursor.slice("offset:".length))
        : 0;
      const filtered = codexThreads
        .filter((thread) => includeArchived || !thread.archived)
        .filter((thread) => {
          if (!term) return true;
          return [
            thread.title,
            thread.id,
            thread.preview,
            thread.model,
            thread.status,
          ].some((value) => String(value ?? "").toLowerCase().includes(term));
        })
        .sort((a, b) => (b.updatedAt ?? 0) - (a.updatedAt ?? 0));
      const page = filtered.slice(cursor, cursor + limit);
      const nextOffset = cursor + limit;
      return {
        items: page.map((thread) => ({ ...thread, provider: "codex" })),
        nextCursor: nextOffset < filtered.length ? `offset:${nextOffset}` : null,
      };
    }

    case "history_import_runtime_states": {
      return Object.entries(codexTaskSessions).flatMap(([taskId, threadId]) => {
        const task = tasks.find((row) => row.id === taskId && !row.archived);
        if (!task) return [];
        const queuedCount = chatQueued[taskId]?.length ?? 0;
        const running = chatRunning[taskId] === true;
        return [{
          itemId: threadId,
          taskId,
          taskTitle: task.title,
          projectId: task.projectId,
          running,
          queued: queuedCount > 0,
          pending: running || queuedCount > 0,
          queuedCount,
        }];
      });
    }

    case "history_import_clean_background_terminals": {
      const itemId = String(args.itemId ?? "").trim();
      if (!itemId) throw new Error("Codex threadId 不能为空");
      cleanedCodexThreads.push(itemId);
      return undefined;
    }

    case "router_get_mode": {
      const backend = normalizeBackend(args.backend);
      return routerModes[backend];
    }

    case "router_set_mode": {
      const backend = normalizeBackend(args.backend);
      routerModes[backend] = backend === "codex" && args.mode === "codex-account"
        ? "codex-account"
        : "api";
      return undefined;
    }

    case "agent_timeline_list": {
      if (agentTimelineDelayMs > 0) {
        await new Promise((resolve) => setTimeout(resolve, agentTimelineDelayMs));
      }
      const taskId = String(args.taskId);
      return timelineEvents[taskId] ?? [];
    }

    case "agent_timeline_clear_task": {
      const taskId = String(args.taskId);
      const count = timelineEvents[taskId]?.length ?? 0;
      timelineEvents[taskId] = [];
      return count;
    }

    case "quota_usage_get_stats":
      return quotaUsageStatsOverride ?? createMockQuotaUsageStats(
        args.input && typeof args.input === "object" && !Array.isArray(args.input)
          ? args.input as Record<string, unknown>
          : {},
      );

    case "quota_usage_get_codex_account_status":
      return codexAccountQuotaStatusOverride ?? createMockCodexAccountQuotaStatus();

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

    case "chat_get_runtime_snapshot": {
      const taskId = String(args.taskId);
      const queuedCount = chatQueued[taskId]?.length ?? 0;
      const running = chatRunning[taskId] === true;
      return {
        taskId,
        phase: running && queuedCount > 0
          ? "running_and_queued"
          : running
            ? "running"
            : queuedCount > 0
              ? "queued"
              : "idle",
        backend: running ? activeBackend : null,
        turnId: running ? currentChatTurnId(taskId) : null,
        queuedCount,
        pendingRollback: false,
        pendingResetCleanup: false,
        rollback: null,
        ...(runtimeSnapshotOverrides[taskId] ?? {}),
      };
    }

    case "plugins_overview":
      return {
        skills: [
          {
            backend: "claude",
            scope: "user",
            name: "mock-skill",
            description: "测试用 Skill",
            enabled: true,
            path: "C:\\Users\\mock\\.claude\\skills\\mock-skill\\SKILL.md",
          },
          {
            backend: "claude",
            scope: "project",
            name: "project-skill",
            description: "项目 Skill",
            enabled: true,
            path: "D:\\PROJECT\\workspace\\Lilia\\.claude\\skills\\project-skill\\SKILL.md",
          },
        ],
        packages: claudePlugins.map((plugin) => ({ ...plugin })),
        mcpServers: [
          ...claudeMcpServers.map((server) => ({
            ...server,
            args: [...server.args],
            envKeys: [...server.envKeys],
          })),
          ...codexMcpServers.map((server) => ({
            ...server,
            args: [...server.args],
            envKeys: [...server.envKeys],
          })),
        ],
        configPaths: {
          claude: "C:\\Users\\mock\\.lilia\\config\\claude-mcp-servers.json",
          codex: "C:\\Users\\mock\\.codex\\config.toml",
        },
        warnings: [],
      };

    case "plugins_set_package_enabled": {
      if (args.backend !== "claude") throw new Error("unsupported package backend");
      const name = String(args.name);
      const enabled = args.enabled === true;
      claudePlugins = claudePlugins.map((plugin) =>
        plugin.name === name ? { ...plugin, enabled } : plugin
      );
      return undefined;
    }

    case "plugins_create_mcp_server": {
      const backend = String(args.backend);
      const input = args.input as {
        name?: string;
        command?: string;
        args?: string[];
        env?: Record<string, string>;
      };
      const server = {
        backend,
        name: String(input.name ?? ""),
        command: String(input.command ?? ""),
        args: Array.isArray(input.args) ? input.args.map(String) : [],
        envKeys: Object.keys(input.env ?? {}),
        enabled: true,
        editable: true,
        transport: "stdio",
      };
      updateMcpServersForBackend(backend, (servers) => [...servers, server]);
      return { ...server, args: [...server.args], envKeys: [...server.envKeys] };
    }

    case "plugins_update_mcp_server": {
      const backend = String(args.backend);
      const name = String(args.name);
      const input = args.input as {
        name?: string;
        command?: string;
        args?: string[];
        env?: Record<string, string>;
        removeEnvKeys?: string[];
      };
      const servers = mcpServersForBackend(backend);
      let updated = servers.find((server) => server.name === name);
      if (!updated) return undefined;
      if (backend === "codex" && !updated.editable) return undefined;
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
      updateMcpServersForBackend(backend, (servers) =>
        servers.map((server) => (server.name === name ? updated : server)),
      );
      return { ...updated, args: [...updated.args], envKeys: [...updated.envKeys] };
    }

    case "plugins_delete_mcp_server": {
      const backend = String(args.backend);
      const name = String(args.name);
      updateMcpServersForBackend(backend, (servers) =>
        servers.filter((server) => server.name !== name || (backend === "codex" && !server.editable)),
      );
      return undefined;
    }

    case "plugins_set_mcp_server_enabled": {
      const backend = String(args.backend);
      const name = String(args.name);
      const enabled = args.enabled === true;
      updateMcpServersForBackend(backend, (servers) =>
        servers.map((server) =>
          server.name === name && (backend === "claude" || server.editable)
            ? { ...server, enabled }
            : server,
        ),
      );
      return undefined;
    }

    case "plugins_open_mcp_config":
      return undefined;

    case "chat_set_composer_state":
      return undefined;

    case "agent_interaction_get_settings":
      return { ...agentInteractionSettings };

    case "agent_interaction_set_settings": {
      const settings = args.settings as {
        nonInterruptMode?: unknown;
        debug?: unknown;
        codexProfile?: {
          profile?: unknown;
          model?: unknown;
          reasoningEffort?: unknown;
          runtimeWorkspaceRoots?: unknown;
          permissions?: { profile?: unknown };
          responsesApiClientMetadata?: unknown;
          additionalContext?: unknown;
          persistExtendedHistory?: unknown;
          initialTurnsPage?: unknown;
          excludeTurns?: unknown;
        };
      } | undefined;
      const codexProfile = settings?.codexProfile;
      agentInteractionSettings = {
        nonInterruptMode: settings?.nonInterruptMode === true,
        debug: settings?.debug === true,
        codexProfile: {
          profile: typeof codexProfile?.profile === "string" ? codexProfile.profile : "default",
          model: typeof codexProfile?.model === "string" && codexProfile.model.trim()
            ? codexProfile.model.trim()
            : null,
          reasoningEffort: typeof codexProfile?.reasoningEffort === "string"
            ? codexProfile.reasoningEffort
            : null,
          runtimeWorkspaceRoots: Array.isArray(codexProfile?.runtimeWorkspaceRoots)
            ? codexProfile.runtimeWorkspaceRoots.map(String)
            : [],
          responsesApiClientMetadata: normalizeMockJsonObject(codexProfile?.responsesApiClientMetadata),
          additionalContext: typeof codexProfile?.additionalContext === "string" && codexProfile.additionalContext.trim()
            ? codexProfile.additionalContext.trim()
            : null,
          persistExtendedHistory: typeof codexProfile?.persistExtendedHistory === "boolean"
            ? codexProfile.persistExtendedHistory
            : null,
          initialTurnsPage: normalizeMockJsonObject(codexProfile?.initialTurnsPage),
          excludeTurns: normalizeMockStringList(codexProfile?.excludeTurns),
        },
      };
      return undefined;
    }

    case "chat_list_models": {
      const backend = String(args.backend || "claude");
      if (backend === "codex") {
        return CODEX_MODEL_OPTIONS.map((option) => ({ ...option, backend }));
      }
      return [
        {
          id: "claude-sonnet-4-6",
          label: "Sonnet 4.6",
          backend,
        },
      ];
    }

    case "chat_respond_agent_interaction":
      if (nextAgentInteractionResponseError) {
        const message = nextAgentInteractionResponseError;
        nextAgentInteractionResponseError = null;
        throw new Error(message);
      }
      return undefined;
    case "project_architecture_apply": {
      const input = args.input && typeof args.input === "object" && !Array.isArray(args.input)
        ? args.input as Record<string, unknown>
        : {};
      const graph = {
        projectId: String(input.projectId ?? "lilia"),
        version: 1,
        summary: "",
        nodes: [],
        edges: [],
        updatedAt: Date.now(),
      };
      return {
        graph,
        event: {
          id: typeof input.requestId === "string" ? input.requestId : "architecture-1",
          projectId: graph.projectId,
          taskId: String(input.taskId ?? ""),
          turnId: typeof input.turnId === "string" ? input.turnId : null,
          backend: input.backend === "codex" ? "codex" : "claude",
          permission: typeof input.permission === "string" ? input.permission : "ask",
          status: "applied",
          reason: typeof input.reason === "string" ? input.reason : "",
          changes: Array.isArray(input.changes) ? input.changes : [],
          beforeVersion: 0,
          afterVersion: 1,
          createdAt: Date.now(),
          resolvedAt: Date.now(),
        },
      };
    }
    case "chat_ack_restored_rollback":
      return undefined;

    case "chat_respond_title_update": {
      const taskId = String(args.taskId);
      const requestId = String(args.requestId);
      const decision = args.decision === "accept" ? "accept" : "decline";
      const eventId = `title-update:${taskId}:${requestId}`;
      const event = (timelineEvents[taskId] ?? []).find((item) =>
        item.id === eventId && item.kind === "title_update"
      );
      if (!event || event.status !== "requires_action") return undefined;
      const payload = event.payload && typeof event.payload === "object" && !Array.isArray(event.payload)
        ? event.payload as Record<string, unknown>
        : {};
      const proposedTitle = typeof payload.proposedTitle === "string"
        ? payload.proposedTitle
        : event.summary ?? "";
      if (decision === "accept" && proposedTitle.trim()) {
        let changedProjectId: string | null = null;
        tasks = tasks.map((task) => {
          if (task.id !== taskId || task.archived) return task;
          changedProjectId = task.projectId;
          return { ...task, title: proposedTitle, titleSource: "manual" };
        });
        emitTauriEvent("tasks:changed", { projectId: changedProjectId });
      }
      emitMockTimelineEvent(taskId, {
        ...event,
        status: decision === "accept" ? "success" : "skipped",
        payload: {
          ...payload,
          accepted: decision === "accept",
          decision,
        },
        updatedAt: Date.now(),
      });
      return undefined;
    }

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

    case "chat_save_clipboard_text": {
      clipboardTextSeq += 1;
      const input = args.input && typeof args.input === "object" && !Array.isArray(args.input)
        ? args.input as Record<string, unknown>
        : {};
      const text = typeof input.text === "string" ? input.text : "";
      const path = `C:\\Users\\mock\\.lilia\\cache\\clipboard-texts\\clipboard-${clipboardTextSeq}.txt`;
      return {
        id: `clip-text-${clipboardTextSeq}`,
        name: `粘贴文本 ${clipboardTextSeq}.txt`,
        path,
        kind: "file",
        size: new TextEncoder().encode(text).length,
        exists: true,
        mime: null,
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

    case "chat_search_slash_commands": {
      const query = String(args.query ?? "").trim().toLowerCase();
      const limit = typeof args.limit === "number" ? args.limit : 12;
      return mockSlashCommands
        .filter((command) =>
          !query ||
          command.name.includes(query) ||
          command.title.toLowerCase().includes(query) ||
          command.description.toLowerCase().includes(query)
        )
        .slice(0, limit)
        .map((command) => ({
          command: { ...command, parameters: [...command.parameters] },
          matchedBy: !query || command.name.includes(query) ? "name" : "title",
        }));
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
      if (nextChatSendError) {
        const message = nextChatSendError;
        nextChatSendError = null;
        throw new Error(message);
      }
      const taskId = String(args.taskId);
      const content = String(args.content);
      const composer = normalizeComposer(args.composer, taskId);
      args.composer = composer;
      const attachments = Array.isArray(args.attachments) ? args.attachments : [];
      const workflow = args.workflow && typeof args.workflow === "object" && !Array.isArray(args.workflow)
        ? args.workflow as Record<string, unknown>
        : null;
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
      if (workflow?.type === "slash_command" && typeof workflow.commandId === "string") {
        const command = mockSlashCommands.find((item) => item.id === workflow.commandId);
        if (!command) throw new Error(`未知斜杠命令：${workflow.commandId}`);
        const projectCwd = String(args.projectCwd ?? "D:\\PROJECT\\workspace\\Lilia");
        const output = mockSlashCommandOutput(command, projectCwd, composer.backend);
        emitMockTimelineEvent(taskId, {
          id: `${taskId}:${turnId}:slash-command`,
          turnId,
          kind: "command",
          backend: composer.backend,
          status: "success",
          title: `/${command.name}`,
          summary: output,
          payload: {
            command: `/${command.name}`,
            source: command.source,
            title: command.title,
            output,
            exitCode: 0,
            subkind: "slash_command",
          },
          createdAt: message.createdAt,
          updatedAt: message.createdAt,
          turnSeq: 0,
          intraTurnOrder: 0,
        });
        queueMicrotask(() => {
          emitTauriEvent("chat:done", {
            taskId,
            sessionId: null,
            subtype: "slash_command",
            rollback: null,
          });
        });
        return {
          message,
          dispatch: "started",
          queuedCount: 0,
          turnId,
        };
      }
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
          turnId,
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
        turnId,
      };
    }

    case "lilia_iab_open":
      return undefined;

    case "lilia_iab_submit": {
      const snapshot = createMockLiliaIabSnapshot(args);
      const delivery = nextLiliaIabDelivery;
      nextLiliaIabDelivery = "message";
      return {
        snapshot,
        delivery,
        stdinForwarded: delivery === "runner",
      };
    }

    case "chat_interrupt_turn": {
      const taskId = String(args.taskId);
      const turnId = currentChatTurnId(taskId);
      const message = "用户打断了当前 Agent 运行";
      resetMockQueuedGuides(chatQueued[taskId]);
      chatQueued[taskId] = [];
      if (chatRunning[taskId] === true) {
        const turnEvents = (timelineEvents[taskId] ?? []).filter((event) =>
          event.turnId === turnId
        );
        const [onlyEvent] = turnEvents;
        const payload = onlyEvent?.payload as Record<string, unknown> | null;
        if (
          turnEvents.length === 1 &&
          onlyEvent?.kind === "message" &&
          payload?.role === "user"
        ) {
          timelineEvents[taskId] = (timelineEvents[taskId] ?? []).filter((event) =>
            event.id !== onlyEvent.id
          );
          chatRunning[taskId] = false;
          queueMicrotask(() => {
            emitTauriEvent("chat:done", {
              taskId,
              sessionId: null,
              subtype: null,
              rollback: {
                rolledBack: true,
                restoredContent: typeof payload.content === "string" ? payload.content : "",
                restoredAttachments: Array.isArray(payload.attachments) ? payload.attachments : [],
                removedEventIds: [onlyEvent.id],
              },
            });
          });
          return {
            rolledBack: false,
            restoredContent: "",
            restoredAttachments: [],
            removedEventIds: [],
          };
        }
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
            rollback: null,
          });
        });
      }
      return {
        rolledBack: false,
        restoredContent: "",
        restoredAttachments: [],
        removedEventIds: [],
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
