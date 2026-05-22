/**
 * Lilia 共享契约：前端 / Tauri Rust / 其他工作区包共用的数据模型。
 *
 * - Project：对应 Claude Code 的某个工作目录（cwd）。
 * - Session：Claude Code 真实写入磁盘的会话原始记录。
 * - Task：Lilia 在 Session 之上叠加的视图层概念，可以拥有父任务和前置任务。
 *   一个 Task 永远绑定一个 Session。
 */

export interface Project {
  id: string;
  name: string;
  /** Claude Code 工作目录的绝对路径；分类型项目（仅做归类用）为 null。 */
  cwd: string | null;
  /** 该项目下的会话数量（用于侧边栏角标）。 */
  sessionCount: number;
}

export type SessionKind = "interactive" | "headless" | "unknown";

export interface Session {
  /** Claude Code 自身的会话 UUID。 */
  sessionId: string;
  projectId: string;
  cwd: string;
  startedAt: number;
  kind: SessionKind;
  /** 是否仍有活跃进程。 */
  alive: boolean;
}

export type TaskStatus =
  | "draft"
  | "waiting"
  | "running"
  | "blocked"
  | "done"
  | "cancelled";

export interface Task {
  id: string;
  projectId: string;
  /** 绑定的 Claude Code 会话 ID。 */
  sessionId: string;
  title: string;
  status: TaskStatus;
  createdAt: number;
  /** 父任务 id，null 表示顶层任务。 */
  parentId: string | null;
  /** 必须先完成的前置任务 id 列表。 */
  dependsOn: string[];
}

export interface TaskGraph {
  tasks: Task[];
  /** 由 id 指向其直接子任务 id 的索引。 */
  childrenByParent: Record<string, string[]>;
}

/**
 * Lilia v1 在 Task / Session 之上叠加的三组结构化资产，对应
 * 颗粒度金字塔 Milestone → Task → Todo → Message 与横向 Memory：
 *
 * - TaskTodo：任务内的 checklist，定位为「AI 思考过程可视化」，
 *   主要由 Claude SDK 的 TodoWrite 工具事件自动 upsert（source = "agent"）。
 * - Memory：跨会话沉淀，user / project 两层，发送时拼为 system 前缀注入。
 * - Milestone + TaskMilestoneLink：项目长跨度叙事；不嵌套，Task 可游离。
 *
 * 设计原则见 [[lilia-v1-feature-architecture]]。
 */

export type TaskTodoSource = "user" | "agent";

export interface TaskTodo {
  id: string;
  taskId: string;
  text: string;
  done: boolean;
  /** 列表内排序，越小越靠前。 */
  order: number;
  /** "agent" 表示来自 Claude SDK TodoWrite；"user" 是手动维护。 */
  source: TaskTodoSource;
  createdAt: number;
  updatedAt: number;
}

export type MemoryScope = "user" | "project";

export interface Memory {
  id: string;
  scope: MemoryScope;
  /** scope = "user" 时为 null。 */
  projectId: string | null;
  title: string;
  /** Markdown 主体。 */
  body: string;
  tags: string[];
  /** 是否参与对话发送时的 prompt 注入。 */
  enabled: boolean;
  /**
   * 起源任务的 id；手动添加时为 null。
   * 草稿期保存的记忆用 `"draft:<draftId>"` 字符串记录来源，
   * 与正式 task id 区分但保持可追溯。
   */
  sourceTaskId: string | null;
  createdAt: number;
  updatedAt: number;
}

export type MilestoneStatus =
  | "upcoming"
  | "in-progress"
  | "done"
  | "abandoned";

export interface Milestone {
  id: string;
  projectId: string;
  /** 例如 "v1.0 公测"。 */
  title: string;
  /** Markdown 描述，可为空字符串。 */
  description: string;
  status: MilestoneStatus;
  /** 截止日期；长期愿景型里程碑可为 null。 */
  dueDate: number | null;
  /** 路线图上的纵向排序，越小越靠前。 */
  order: number;
  createdAt: number;
}

/**
 * Task ↔ Milestone 多对多关联。Task 可游离（无任何 link），
 * 路线图视图把游离任务收进底部「未归类」分组。
 */
export interface TaskMilestoneLink {
  taskId: string;
  milestoneId: string;
}

/**
 * 聊天面板契约。ChatMessage 只覆盖纯文本，引入 tool_use / 图像时改成 discriminated union。
 * ChatComposerState 按 taskId 持久化，切换会话不会污染彼此的偏好。
 */

export type ChatRole = "user" | "assistant" | "system";

export interface ChatMessage {
  id: string;
  taskId: string;
  role: ChatRole;
  content: string;
  createdAt: number;
}

export type PermissionMode = "full" | "ask" | "readonly";

/**
 * 当前支持的对话后端：
 * - claude：@anthropic-ai/claude-agent-sdk
 * - codex：@openai/codex-sdk（内部 spawn 本地 codex CLI）
 */
export type ChatBackendKind = "claude" | "codex";

export interface ChatComposerState {
  taskId: string;
  backend: ChatBackendKind;
  /** 模型 id 的语义由 backend 决定（claude-* / gpt-5-codex 等）。 */
  model: string;
  /** 当前 git 分支名。 */
  branch: string;
  permission: PermissionMode;
}

export interface ChatModelOption {
  id: string;
  label: string;
  backend: ChatBackendKind;
}

export interface ChatBranchOption {
  name: string;
  current: boolean;
}

/**
 * 单个 backend 的路由模式。每个 backend 独立选择，可分别走不同路由。
 * - cc-switch：经 CCSwitchConfig 的代理 URL 转发
 * - direct：用 ProviderConfig 的 baseUrl + apiKey 直连真实 API
 */
export type RouterMode = "cc-switch" | "direct";

/**
 * 单 backend 的直连配置：仅在该 backend 的 router 为 "direct" 时被读取。
 */
export interface ProviderConfig {
  backend: ChatBackendKind;
  baseUrl: string | null;
  apiKey: string | null;
}

/**
 * 项目相关偏好。一期只放 git clone 的默认父目录；后续项目级偏好都挂这里。
 */
export interface ProjectSettings {
  /** 「添加项目 → 从 GitHub clone」默认 clone 父目录；为空时 UI 兜底用家目录。 */
  cloneParentDir: string | null;
}

/**
 * CC-Switch 代理层配置：所有走 cc-switch 路由的 backend 共用一个代理 URL。
 * 默认 http://127.0.0.1:15721。
 */
export interface CCSwitchConfig {
  baseUrl: string | null;
}

/**
 * 辅助模型（Assistant AI）配置。
 *
 * 独立于 ProviderConfig 的 OpenAI 兼容配置，**不参与 Agent 主循环**，
 * 仅供周边系统消费——例如 Memory 模块的外置观察模型
 * （见 docs/design/memory.md §3.1）、Tool Call 后处理、对话摘要等。
 * 目的：把高频低复杂度的辅助请求迁到便宜模型，降低 Agent 提供商成本。
 *
 * v1 仅全局一份。三件套（baseUrl / apiKey / model）齐全才算启用，
 * 任一缺失消费方应 short-circuit 跳过。
 */
export interface AssistantAIConfig {
  baseUrl: string | null;
  apiKey: string | null;
  model: string | null;
}

/** 连通性测试结果。models 在端点不支持 /models 时为 null，UI 据此降级提示。 */
export interface AssistantAITestResult {
  ok: boolean;
  /** 失败原因；ok=true 时为 null。 */
  error: string | null;
  /** 服务端 /models 返回的 id 列表；端点不支持时为 null。 */
  models: string[] | null;
  /** 配置里的 model 是否出现在 models 列表里。models=null 时为 null。 */
  modelMatched: boolean | null;
}

export type ConnectionMode = "cc-switch" | "custom" | "direct" | "unconfigured";

export interface BackendEnvStatus {
  backend: ChatBackendKind;
  hasApiKey: boolean;
  /** 当前实际走的路由（由 routerMode 决定 + 配置是否完整）。 */
  connectionMode: ConnectionMode;
  /** 兜底说明用的目标 URL；unconfigured 时为 null。 */
  effectiveUrl: string | null;
}

/** CC-Switch 代理层的全局健康信息。 */
export interface CCSwitchStatus {
  /** 单一代理 URL 是否能 TCP 拨通；URL 为空时为 false。 */
  reachable: boolean;
  baseUrl: string | null;
}

export interface EnvStatusReport {
  nodeAvailable: boolean;
  /** codex CLI 是否能在 PATH 找到（@openai/codex-sdk 是 wrapper）。 */
  codexCliAvailable: boolean;
  ccSwitch: CCSwitchStatus;
  /** 每个 backend 当前生效的路由模式。 */
  routerModes: Record<ChatBackendKind, RouterMode>;
  backends: Record<ChatBackendKind, BackendEnvStatus>;
}

/**
 * 插件 / 技能管理契约。两个 backend 扩展机制不对称：
 * - Claude Code：skills (markdown 文件夹) + plugins (marketplace bundle)，
 *   目录 `~/.claude/<kind>/<name>/` 或 `<cwd>/.claude/<kind>/<name>/`。
 *   Lilia 一期直接管理 skill；plugin 仅列出已安装。
 * - Codex：扩展集中在 `~/.codex/config.toml` 的 `[mcp_servers.*]` 节，
 *   一期只读 + 一键打开配置文件。
 */

/** Skill 存放层级：用户级（跨项目共享）/ 项目级（绑定某个 cwd）。 */
export type PluginScope = "user" | "project";

export type PluginBackendKind = "claude" | "codex";

export interface ClaudeSkill {
  scope: PluginScope;
  /** 目录名 / SKILL.md frontmatter 里的 name。 */
  name: string;
  /** SKILL.md frontmatter 里的 description；缺失时是空串。 */
  description: string;
  /**
   * 是否对 Agent SDK 生效。frontmatter 里自定义 `disabled: true` 视为关闭，空缺即启用。
   * Claude 官方不读这个字段，Lilia 在启动 agent 子进程时按它决定 pluginRoots。
   */
  enabled: boolean;
  /** SKILL.md 的绝对路径。 */
  path: string;
}

export interface ClaudePlugin {
  scope: PluginScope;
  /** 插件目录名。 */
  name: string;
  /** plugin.json 里的 description（缺失留空）。 */
  description: string;
  /** plugin.json 里的 version（缺失留空）。 */
  version: string;
  enabled: boolean;
  /** 插件根目录绝对路径。 */
  path: string;
}

export interface CodexMcpServer {
  name: string;
  command: string;
  args: string[];
  /** Codex 没有 disabled 字段；只要节存在就视为启用，UI 仍展示 enabled=true 角标。 */
  enabled: boolean;
}

/** 一次拉全数据的便利接口：UI 启动时一次性 invoke，省一轮 round trip。 */
export interface PluginsOverview {
  claudeUserSkills: ClaudeSkill[];
  claudeProjectSkills: ClaudeSkill[];
  claudeUserPlugins: ClaudePlugin[];
  codexMcpServers: CodexMcpServer[];
  /** 解析期发生的非致命错误，UI 用来提示「读取 .codex/config.toml 时第 N 行有误」。 */
  warnings: string[];
}
