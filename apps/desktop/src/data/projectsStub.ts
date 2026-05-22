/**
 * Barrel re-export — 过渡期保留，所有新代码请直接从 `data/projects`、
 * `data/tasks`、`data/orphans` 导入。
 */

export { makeId } from "./shared";

export {
  listProjects,
  getProject,
  createProject,
  renameProject,
  removeProject,
  deriveProjectName,
} from "./projects";

export {
  listTasks,
  getTask,
  listProjectConversations,
  isDraftTask,
  createDraftTask,
  promoteDraftTask,
  archiveProjectConversations,
} from "./tasks";

export {
  listOrphanConversations,
  getOrphanConversation,
  isDraftOrphan,
  createDraftOrphan,
  promoteDraftOrphan,
} from "./orphans";

export type { OrphanConversation } from "./orphans";
