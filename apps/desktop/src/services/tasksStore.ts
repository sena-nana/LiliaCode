/**
 * 任务 + 草稿/孤儿对话 store：UI 层的「Task / OrphanConversation」全部从这里取。
 *
 * 所有数据经 Tauri IPC 走 SQLite 持久化；签名保持稳定，UI 不动。
 * 组件**只**从 `services/` 导入。
 */

export {
  listTasks,
  getTask,
  listProjectConversations,
  archiveProjectConversations,
  archiveTask,
  isDraftTask,
  createDraftTask,
  promoteDraftTask,
  reorderTasks,
  reparentTask,
} from "../data/tasks";

export {
  listOrphanConversations,
  getOrphanConversation,
  isDraftOrphan,
  createDraftOrphan,
  promoteDraftOrphan,
} from "../data/tasks";

export type { OrphanConversation } from "../data/tasks";
