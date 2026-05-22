/**
 * 任务 + 草稿/孤儿对话 store：UI 层的「Task / OrphanConversation」全部从这里取。
 *
 * 当前实现直接 re-export `data/tasks` 和 `data/orphans` 的内存 ref 版本；后续接
 * SQLite 时仅替换内部实现，签名保持稳定，UI 不动。组件**只**从 `services/` 导入。
 */

export {
  listTasks,
  getTask,
  listProjectConversations,
  archiveProjectConversations,
  isDraftTask,
  createDraftTask,
  promoteDraftTask,
} from "../data/tasks";

export {
  listOrphanConversations,
  getOrphanConversation,
  isDraftOrphan,
  createDraftOrphan,
  promoteDraftOrphan,
} from "../data/orphans";

export type { OrphanConversation } from "../data/orphans";
