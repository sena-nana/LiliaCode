import { invoke } from "@tauri-apps/api/core";
import {
  MEMORY_DELETE_COMMAND,
  MEMORY_GET_INJECTION_STATE_COMMAND,
  MEMORY_GET_SETTINGS_COMMAND,
  MEMORY_LIST_COMMAND,
  MEMORY_RESET_TASK_COOLDOWN_COMMAND,
  MEMORY_SET_ENABLED_COMMAND,
  MEMORY_SET_SETTINGS_COMMAND,
  MEMORY_SET_TASK_ENABLED_COMMAND,
  MEMORY_UPSERT_COMMAND,
  type Memory,
  type MemoryInjectionState,
  type MemorySettings,
  type MemoryUpsertInput,
} from "@lilia/contracts";

export function listMemories(projectId: string | null): Promise<Memory[]> {
  return invoke<Memory[]>(MEMORY_LIST_COMMAND, { projectId });
}

export function upsertMemory(input: MemoryUpsertInput): Promise<Memory> {
  return invoke<Memory>(MEMORY_UPSERT_COMMAND, { input });
}

export function setMemoryEnabled(id: string, enabled: boolean): Promise<Memory> {
  return invoke<Memory>(MEMORY_SET_ENABLED_COMMAND, { id, enabled });
}

export function deleteMemory(id: string): Promise<boolean> {
  return invoke<boolean>(MEMORY_DELETE_COMMAND, { id });
}

export function getMemorySettings(): Promise<MemorySettings> {
  return invoke<MemorySettings>(MEMORY_GET_SETTINGS_COMMAND);
}

export function setMemorySettings(settings: MemorySettings): Promise<void> {
  return invoke<void>(MEMORY_SET_SETTINGS_COMMAND, { settings });
}

export function getMemoryInjectionState(taskId: string): Promise<MemoryInjectionState> {
  return invoke<MemoryInjectionState>(MEMORY_GET_INJECTION_STATE_COMMAND, { taskId });
}

export function setTaskMemoryEnabled(
  taskId: string,
  enabled: boolean,
): Promise<MemoryInjectionState> {
  return invoke<MemoryInjectionState>(MEMORY_SET_TASK_ENABLED_COMMAND, { taskId, enabled });
}

export function resetTaskMemoryCooldown(taskId: string): Promise<MemoryInjectionState> {
  return invoke<MemoryInjectionState>(MEMORY_RESET_TASK_COOLDOWN_COMMAND, { taskId });
}
