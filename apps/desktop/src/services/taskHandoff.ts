import type { ImportedTaskHandoff } from "@lilia/contracts";
import { TASK_HANDOFF_GET_COMMAND } from "@lilia/contracts/taskCommandsContract.mjs";
import { invoke } from "../tauri/runtime";

export function getImportedTaskHandoff(taskId: string): Promise<ImportedTaskHandoff | null> {
  return invoke<ImportedTaskHandoff | null>(TASK_HANDOFF_GET_COMMAND, { taskId });
}
