import { invoke } from "@tauri-apps/api/core";
import type {
  TaskWorktree,
  WORKTREE_ATTACH_TASK_COMMAND,
  WORKTREE_CLEANUP_ARCHIVE_COMMAND,
  WORKTREE_CLEAR_TASK_COMMAND,
  WORKTREE_CREATE_FOR_TASK_COMMAND,
  WORKTREE_GET_FOR_TASK_COMMAND,
  WORKTREE_LIST_COMMAND,
  WORKTREE_MERGE_DELETE_ARCHIVE_COMMAND,
  WorktreeAttachInput,
  WorktreeCreateInput,
  WorktreeListItem,
  WorktreeMergeResult,
} from "@lilia/contracts";

export function listWorktrees(baseRepoPath: string): Promise<WorktreeListItem[]> {
  return invoke<WorktreeListItem[]>(WORKTREE_LIST_COMMAND, { baseRepoPath });
}

export function createWorktreeForTask(input: WorktreeCreateInput): Promise<TaskWorktree> {
  return invoke<TaskWorktree>(WORKTREE_CREATE_FOR_TASK_COMMAND, { input });
}

export function attachWorktreeToTask(input: WorktreeAttachInput): Promise<TaskWorktree> {
  return invoke<TaskWorktree>(WORKTREE_ATTACH_TASK_COMMAND, { input });
}

export function getTaskWorktree(taskId: string): Promise<TaskWorktree | null> {
  return invoke<TaskWorktree | null>(WORKTREE_GET_FOR_TASK_COMMAND, { taskId });
}

export function clearTaskWorktree(taskId: string): Promise<void> {
  return invoke<void>(WORKTREE_CLEAR_TASK_COMMAND, { taskId });
}

export function cleanupArchiveWorktree(taskId: string): Promise<WorktreeMergeResult> {
  return invoke<WorktreeMergeResult>(WORKTREE_CLEANUP_ARCHIVE_COMMAND, { taskId });
}

export function mergeDeleteArchiveWorktree(taskId: string): Promise<WorktreeMergeResult> {
  return invoke<WorktreeMergeResult>(WORKTREE_MERGE_DELETE_ARCHIVE_COMMAND, { taskId });
}
