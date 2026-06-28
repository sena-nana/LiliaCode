import { invoke } from "../tauri/runtime";
import {
  POPUP_FOCUS_MAIN_COMMAND,
  POPUP_GET_WINDOW_SETTINGS_COMMAND,
  POPUP_OPEN_CHILD_QUESTION_COMMAND,
  POPUP_OPEN_NEW_CHAT_COMMAND,
  POPUP_OPEN_TASK_COMMAND,
  POPUP_REMEMBER_LAST_PROJECT_COMMAND,
  POPUP_SET_WINDOW_SETTINGS_COMMAND,
  type PopupWindowSettings,
} from "@lilia/contracts";

export type { PopupWindowSettings };

export function getPopupWindowSettings(): Promise<PopupWindowSettings> {
  return invoke<PopupWindowSettings>(POPUP_GET_WINDOW_SETTINGS_COMMAND);
}

export function setPopupWindowSettings(
  settings: PopupWindowSettings,
): Promise<void> {
  return invoke<void>(POPUP_SET_WINDOW_SETTINGS_COMMAND, { settings });
}

export function rememberPopupLastProject(projectId: string): Promise<void> {
  return invoke<void>(POPUP_REMEMBER_LAST_PROJECT_COMMAND, { projectId });
}

export function openPopupNewChat(
  projectId?: string | null,
  initialDraftContent?: string | null,
): Promise<void> {
  return invoke<void>(POPUP_OPEN_NEW_CHAT_COMMAND, {
    projectId: projectId ?? null,
    initialDraftContent: initialDraftContent ?? null,
  });
}

export function openPopupTask(
  taskId: string,
  projectId?: string | null,
): Promise<void> {
  return invoke<void>(POPUP_OPEN_TASK_COMMAND, {
    projectId: projectId ?? null,
    taskId,
  });
}

export function openPopupChildQuestion(
  parentTaskId: string,
  projectId?: string | null,
): Promise<void> {
  return invoke<void>(POPUP_OPEN_CHILD_QUESTION_COMMAND, {
    parentTaskId,
    projectId: projectId ?? null,
  });
}

export function focusMainWindow(route: string): Promise<void> {
  return invoke<void>(POPUP_FOCUS_MAIN_COMMAND, { route });
}

