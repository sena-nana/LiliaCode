/**
 * 项目相关 store：listProjects / getProject / createProject / deriveProjectName。
 *
 * 所有数据经 Tauri IPC 走 SQLite 持久化；签名保持稳定，UI 不动。
 * 组件**只**从 `services/` 导入。
 */

export {
  listProjects,
  getProject,
  createProject,
  renameProject,
  removeProject,
  deriveProjectName,
  reorderProjects,
  toggleProjectPin,
  PROJECTS,
} from "../data/projects";

export { archiveProjectConversations } from "../data/tasks";
