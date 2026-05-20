/**
 * Lilia 共享契约：在前端、Tauri Rust 层及其他工作区包之间共用的数据模型。
 *
 * 设计要点
 * - Project：对应 Claude Code 的某个工作目录（cwd）。
 * - Session：Claude Code 真实写入磁盘的会话原始记录。
 * - Task：Lilia 在 Session 之上叠加的视图层概念，可以拥有父任务和前置任务，
 *   表达「子任务 / 依赖」语义。一个 Task 永远绑定一个 Session。
 */

export interface Project {
  id: string;
  name: string;
  /** Claude Code 工作目录的绝对路径。 */
  cwd: string;
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
 * 聊天面板相关契约。
 *
 * - ChatMessage：单条消息，先只覆盖纯文本；引入 tool_use / 图像 / 工具结果时把
 *   content 改成 discriminated union 即可，外层签名不变。
 * - ChatComposerState：composer 底部那一排可调项（模型 / 分支 / 权限），按
 *   taskId 持久化，切换会话不会污染彼此的偏好。
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
 * - claude：通过 @anthropic-ai/claude-agent-sdk
 * - codex：通过 @openai/codex-sdk（内部 spawn 本地 codex CLI）
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
 * 单个 backend 的路由模式。两选一：
 * - cc-switch：经下方 CCSwitchConfig 的代理 URL 转发
 * - direct：用 ProviderConfig 里的 baseUrl + apiKey 直连真实 API
 *
 * 每个 backend 独立选择——Claude 和 Codex 可以分别用不同的路由。
 */
export type RouterMode = "cc-switch" | "direct";

/**
 * 单 backend 的直连配置：仅在该 backend 的 router 为 "direct" 时被读取。
 * apiKey / baseUrl 为 null 时视为未配置。
 */
export interface ProviderConfig {
  backend: ChatBackendKind;
  baseUrl: string | null;
  apiKey: string | null;
}

/**
 * CC-Switch 代理层配置：所有走 cc-switch 路由的 backend 共用一个代理 URL。
 * 这里假定 CC-Switch 在同一个端口同时承载 Anthropic 和 OpenAI 兼容流量
 * （或上层网关已合并），需要分离时再回到 per-backend 字段。
 * 默认 http://127.0.0.1:15721。
 */
export interface CCSwitchConfig {
  baseUrl: string | null;
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
