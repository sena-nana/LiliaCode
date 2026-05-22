/**
 * 项目相关 store：listProjects / getProject / createProject / deriveProjectName。
 *
 * 当前实现直接 re-export `data/projectsStub` 的内存 ref 版本；后续接 SQLite 时
 * 仅替换内部实现，签名保持稳定，UI 不动。组件**只**从 `services/` 导入。
 */

export {
  listProjects,
  getProject,
  createProject,
  renameProject,
  archiveProjectConversations,
  removeProject,
  deriveProjectName,
} from "../data/projectsStub";
