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

export interface ChatComposerState {
  taskId: string;
  /** 例如 "claude-sonnet-4-6"，由 chat_list_models 返回值约束。 */
  model: string;
  /** 当前 git 分支名。 */
  branch: string;
  permission: PermissionMode;
}

export interface ChatModelOption {
  id: string;
  label: string;
}

export interface ChatBranchOption {
  name: string;
  current: boolean;
}
