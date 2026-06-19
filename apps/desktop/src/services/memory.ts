import { invoke } from "@tauri-apps/api/core";
import type {
  Memory,
  MemoryInjectionState,
  MemorySettings,
  MemoryUpsertInput,
} from "@lilia/contracts";

export function listMemories(projectId: string | null): Promise<Memory[]> {
  return invoke<Memory[]>("memory_list", { projectId });
}

export function upsertMemory(input: MemoryUpsertInput): Promise<Memory> {
  return invoke<Memory>("memory_upsert", { input });
}

export function setMemoryEnabled(id: string, enabled: boolean): Promise<Memory> {
  return invoke<Memory>("memory_set_enabled", { id, enabled });
}

export function deleteMemory(id: string): Promise<boolean> {
  return invoke<boolean>("memory_delete", { id });
}

export function getMemorySettings(): Promise<MemorySettings> {
  return invoke<MemorySettings>("memory_get_settings");
}

export function setMemorySettings(settings: MemorySettings): Promise<void> {
  return invoke<void>("memory_set_settings", { settings });
}

export function getMemoryInjectionState(taskId: string): Promise<MemoryInjectionState> {
  return invoke<MemoryInjectionState>("memory_get_injection_state", { taskId });
}

export function setTaskMemoryEnabled(
  taskId: string,
  enabled: boolean,
): Promise<MemoryInjectionState> {
  return invoke<MemoryInjectionState>("memory_set_task_enabled", { taskId, enabled });
}

export function resetTaskMemoryCooldown(taskId: string): Promise<MemoryInjectionState> {
  return invoke<MemoryInjectionState>("memory_reset_task_cooldown", { taskId });
}
