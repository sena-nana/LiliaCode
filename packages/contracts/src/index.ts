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
  /** 是否置顶。置顶项目显示在列表最上方。 */
  pinned: boolean;
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
  /** 是否置顶。置顶 session 显示在同组列表最上方。 */
  pinned: boolean;
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
 * 聊天命令契约。ChatMessage 只作为发送确认和历史兼容结构；
 * 可见对话流以 AgentTimelineEvent 为唯一来源。
 * ChatComposerState 按 taskId 持久化，切换会话不会污染彼此的偏好。
 */

export type ChatRole = "user" | "assistant" | "system";

export type ChatAttachmentKind = "file" | "directory" | "unknown";

export interface ChatAttachment {
  id: string;
  name: string;
  path: string;
  kind: ChatAttachmentKind;
  size: number | null;
}

export interface ChatMessage {
  id: string;
  taskId: string;
  role: ChatRole;
  content: string;
  attachments: ChatAttachment[];
  createdAt: number;
}

export type ChatSendDispatch = "started" | "queued";

export interface ChatSendResult {
  message: ChatMessage;
  /**
   * started：本次输入立即进入 Agent turn；
   * queued：当前 turn 仍在运行，输入已进入 Lilia 调度队列。
   */
  dispatch: ChatSendDispatch;
  queuedCount: number;
}

export type PermissionMode = "full" | "ask" | "readonly";

/**
 * 当前支持的对话后端：
 * - claude：@anthropic-ai/claude-agent-sdk
 * - codex：@openai/codex-sdk（内部 spawn 本地 codex CLI）
 */
export type ChatBackendKind = "claude" | "codex";

/**
 * 对话时间线。用户输入、工具调用、计划推进、模型最终回复和错误都进入
 * 同一套 timeline；事件生产方通过 display 声明通用渲染语义。
 */
export type AgentTimelineKnownEventKind =
  | "message"
  | "reasoning"
  | "plan"
  | "todo_list"
  | "tool"
  | "ask_user"
  | "command"
  | "subagent"
  | "file_change"
  | "file_read"
  | "search"
  | "web_fetch"
  | "mcp"
  | "web_search"
  | "error"
  | "turn";

export type AgentTimelineEventKind = AgentTimelineKnownEventKind | (string & {});

export type AgentTimelineEventStatus =
  | "pending"
  | "started"
  | "running"
  | "in_progress"
  | "completed"
  | "done"
  | "success"
  | "failed"
  | "error"
  | "cancelled"
  | "skipped"
  | "info"
  | "requires_action";

export type AgentTimelinePayload =
  | null
  | boolean
  | number
  | string
  | AgentTimelinePayload[]
  | { [key: string]: AgentTimelinePayload };

/**
 * 时间线节点图标：lucide 图标的 kebab-case 名（如 "terminal"、"file-pen"、
 * "book-open"），前端动态查找对应组件，解析不到不渲染。
 *
 * NB：display 整套结构（icon / label / action / details / group ...）是渲染时
 * 派生出来的「视图缓存」，不持久化到 DB。事件生产方只负责存事实
 * （`kind` + `payload`），UI 调 `deriveTimelineDisplay()` 现算 —— 这样修改 display
 * 规则可以立即反映到历史事件上。详见 packages/contracts/src/timelineDisplay.ts。
 */
export type AgentTimelineDisplayIcon = string;

export type AgentTimelineDisplayBucket =
  | "command"
  | "file"
  | "plan"
  | "todo"
  | "tool"
  | "ask_user"
  | "mcp"
  | "search"
  | "web_search"
  | "subagent"
  | "error"
  | "other"
  | (string & {});

export interface AgentTimelineDisplayGroup {
  key: string;
  bucket?: AgentTimelineDisplayBucket | null;
  unit?: string | null;
  count?: number | null;
}

export interface AgentTimelineDisplayField {
  label: string;
  value: string;
}

export interface AgentTimelineDisplayListItem {
  text: string;
  tone?: "default" | "muted" | "success" | "warning" | "error" | null;
}

export type AgentTimelineDisplayDetail =
  | {
      type: "line";
      text: string;
      tone?: "default" | "muted" | null;
    }
  | {
      type: "fields";
      fields: AgentTimelineDisplayField[];
    }
  | {
      type: "code";
      label?: string | null;
      content: string;
      language?: string | null;
    }
  | {
      type: "markdown";
      content: string;
      tone?: "default" | "muted" | null;
      singleLine?: boolean | null;
    }
  | {
      type: "list";
      items: AgentTimelineDisplayListItem[];
      ordered?: boolean | null;
    };

export interface AgentTimelineDisplay {
  icon?: AgentTimelineDisplayIcon | null;
  label?: string | null;
  action?: string | null;
  object?: string | null;
  /**
   * action 是泛指动词（"调用工具"、"调用 MCP"）时设 true，渲染层会把 object 拼到标题里
   * （"已调用工具 Edit"），否则只显示动词（"已读取"），object 留给 aria-label/详情用。
   */
  objectInLabel?: boolean | null;
  preview?: string | null;
  details?: AgentTimelineDisplayDetail[] | null;
  group?: AgentTimelineDisplayGroup | null;
  defaultExpanded?: boolean | null;
}

export interface AgentTimelineEvent {
  id: string;
  taskId: string;
  turnId: string | null;
  backend: ChatBackendKind;
  kind: AgentTimelineEventKind;
  status: AgentTimelineEventStatus;
  title: string;
  summary: string | null;
  payload: AgentTimelinePayload;
  createdAt: number;
  updatedAt: number;
  /**
   * task 内 turn 的单调序号，按 turn_id 首次出现分配。turn 之间按 turn_seq 排序，
   * 彻底与 createdAt / order 解耦——这是把排序"按 turn 隔离"的核心：sourceId
   * 重叠最坏只能让同 turn 内事件互覆盖位置，不可能再跨 turn 跳位。
   */
  turnSeq: number;
  /** turn 内事件的落库序号，按 (taskId, turnSeq) 单调递增。 */
  intraTurnOrder: number;
}

export { deriveTimelineDisplay } from "./timelineDisplay";
export type { TimelineDisplayInput } from "./timelineDisplay";

export interface ChatComposerState {
  taskId: string;
  backend: ChatBackendKind;
  /** 模型 id 的语义由 backend 决定（claude-* / gpt-5-codex 等）。 */
  model: string;
  /** 是否让本轮先进入 agent 原生计划模式；确认后仍按 permission 执行。 */
  planMode: boolean;
  permission: PermissionMode;
}

export interface ChatModelOption {
  id: string;
  label: string;
  backend: ChatBackendKind;
}

/**
 * Agent 交互偏好。全局生效，不跟随单个 task 的 composer state。
 */
export interface AgentInteractionSettings {
  /** 开启后，权限请求、Agent 提问和计划确认不再占用输入框。 */
  nonInterruptMode: boolean;
  /** 开启后，任务页侧边栏注册调试面板，允许注入预制 timeline 事件。 */
  debug: boolean;
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

/**
 * Agent 向用户发起的"求确认 / 求选择"协议。一次 askUser 调用可以排进 1~N 个问题，
 * UI 端按顺序逐一展示；每个问题独立有 mode（confirm / single / multi）和回答。
 *
 * 设计要点：
 * - 与现有 dialog（ConfirmDialog / CategoryDialog）共用 search-palette 浮层壳，
 *   组件视觉以 dialog__card + dialog__body 几何为基；本协议只描述「问什么」。
 * - 选项语义对齐 AskUserQuestion 工具：每个 option 给 label + 可选 description；
 *   recommended/danger/icon 仅作样式提示，UI 端可自由降级。
 * - 多问题时 UI 应保持单卡片、步进显示（progress 1/N），不要堆叠 N 个浮层。
 * - 回答最终以 questionId → AskUserAnswer 的映射回收，UI 自带"上一题"回滚。
 */

export type AskUserMode = "confirm" | "single" | "multi";

export interface AskUserOption {
  /** 选项 ID。返回时写进 AskUserAnswer.value；缺省时用 label。 */
  id?: string;
  /** 选项主标题。 */
  label: string;
  /** 1~2 句描述，渲染在 label 下方的副文本。 */
  description?: string;
  /** focus 当前选项时在右侧预览区渲染的等宽文本（mockup / 代码片段）。 */
  preview?: string;
  /** lucide 图标 kebab-case 名；缺失不渲染。 */
  icon?: string;
  /** 标记为推荐项：渲染时优先 focus + 附 "推荐" 角标。 */
  recommended?: boolean;
  /** 危险动作样式：选项框染红，并要求二次确认。 */
  danger?: boolean;
}

export interface AskUserQuestion {
  /** 稳定 id：回答 map 里用它做 key；同一次 askUser 内不能重复。 */
  id: string;
  /** 顶部小角标（≤12 字符），例如 "认证方式"。可空。 */
  header?: string;
  /** 问题主文本。允许包含 markdown，UI 端按 MarkdownBlock 渲染。 */
  question: string;
  /**
   * confirm：是 / 否（不需要 options）
   * single：单选，options 必填，2~6 项
   * multi：多选，options 必填，2~10 项
   */
  mode: AskUserMode;
  options?: AskUserOption[];
  /** confirm 模式的主按钮文案；缺省 "确认"。 */
  confirmLabel?: string;
  /** confirm 模式的次按钮文案；缺省 "取消"。 */
  cancelLabel?: string;
  /** 整题以危险动作呈现：标题加 AlertTriangle，主按钮换 ghost danger。 */
  danger?: boolean;
  /** 是否允许跳过本题（保留为 skipped）。多问题流程默认 true，单问题默认 false。 */
  skippable?: boolean;
  /** single/multi 是否允许"其他…"自填文本。 */
  allowOther?: boolean;
  /** multi 模式：要求至少选 N 个；缺省 1。 */
  minSelections?: number;
  /** multi 模式：最多选 N 个；缺省不限。 */
  maxSelections?: number;
}

export interface AskUserSpec {
  /** 顶部标题，例如 "Lilia 想确认几件事"；缺省按问题数自动生成。 */
  title?: string;
  /** 来源标签，渲染在标题旁的小 chip，例如 agent 名 / 工具名。 */
  source?: string;
  /** 特定业务意图；缺省按普通 ask-user 处理。 */
  intent?: "plan_approval";
  /** 是否允许整体取消（按 Esc / 点击遮罩 / 右上 X 关闭）；默认 true。 */
  dismissable?: boolean;
  questions: AskUserQuestion[];
}

export interface AskUserAnswer {
  questionId: string;
  /**
   * confirm：'yes' | 'no'
   * single：所选 option.id；选了 "其他" 则为 'other'
   * multi： option.id 数组；含 'other' 时配合 notes
   */
  value: "yes" | "no" | "other" | string | string[];
  /** allowOther 时用户填的自定义文本；plan_approval 场景承载计划修改要求。 */
  notes?: string;
  /** 用户主动跳过本题。 */
  skipped?: boolean;
}

export interface AskUserResult {
  /** questionId → AskUserAnswer。被跳过 / 取消的题不会出现在这里。 */
  answers: Record<string, AskUserAnswer>;
  /** 用户在过程中关闭了整个对话。answers 仍包含此前已回答的题。 */
  cancelled: boolean;
}

export interface AgentAskUserRequestEvent {
  taskId: string;
  turnId: string;
  backend: ChatBackendKind;
  requestId: string;
  spec: AskUserSpec;
}

export interface AgentAskUserResponsePayload {
  taskId: string;
  requestId: string;
  result: AskUserResult;
}
