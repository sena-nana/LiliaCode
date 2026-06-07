import { invoke } from "@tauri-apps/api/core";
import type { PopupWindowSettings } from "@lilia/contracts";

export type { PopupWindowSettings };

export function getPopupWindowSettings(): Promise<PopupWindowSettings> {
  return invoke<PopupWindowSettings>("popup_get_window_settings");
}

export function setPopupWindowSettings(
  settings: PopupWindowSettings,
): Promise<void> {
  return invoke<void>("popup_set_window_settings", { settings });
}

export function rememberPopupLastProject(projectId: string): Promise<void> {
  return invoke<void>("popup_remember_last_project", { projectId });
}

export function openPopupNewChat(
  projectId?: string | null,
  initialDraftContent?: string | null,
): Promise<void> {
  return invoke<void>("popup_open_new_chat", {
    projectId: projectId ?? null,
    initialDraftContent: initialDraftContent ?? null,
  });
}

export function openPopupTask(
  taskId: string,
  projectId?: string | null,
): Promise<void> {
  return invoke<void>("popup_open_task", {
    projectId: projectId ?? null,
    taskId,
  });
}

export function focusMainWindow(route: string): Promise<void> {
  return invoke<void>("popup_focus_main", { route });
}
