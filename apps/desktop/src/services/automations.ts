import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AutomationRun,
  AutomationRunNodeState,
  AutomationRunOnceInput,
  AutomationResumeRunInput,
  AutomationSaveDraftInput,
  AutomationWorkflow,
  AutomationWorkflowVersion,
} from "@lilia/contracts";

export type {
  AutomationRun,
  AutomationRunNodeState,
  AutomationRunOnceInput,
  AutomationResumeRunInput,
  AutomationSaveDraftInput,
  AutomationWorkflow,
  AutomationWorkflowVersion,
};

export interface AutomationRunDetail {
  run: AutomationRun;
  nodes: AutomationRunNodeState[];
}

interface AutomationChangedEvent {
  workflowId: string | null;
}

interface AutomationRunEvent {
  run: AutomationRun;
}

export function listAutomations(): Promise<AutomationWorkflow[]> {
  return invoke<AutomationWorkflow[]>("automation_list_workflows");
}

export function saveAutomationDraft(
  input: AutomationSaveDraftInput,
): Promise<AutomationWorkflow> {
  return invoke<AutomationWorkflow>("automation_save_draft", { input });
}

export function publishAutomation(id: string): Promise<AutomationWorkflowVersion> {
  return invoke<AutomationWorkflowVersion>("automation_publish", { id });
}

export function setAutomationEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke<void>("automation_set_enabled", { id, enabled });
}

export function runAutomationOnce(
  id: string,
  input: AutomationRunOnceInput = {},
): Promise<AutomationRun> {
  return invoke<AutomationRun>("automation_run_once", { id, input });
}

export function resumeAutomationRun(
  runId: string,
  input: AutomationResumeRunInput = {},
): Promise<AutomationRun> {
  return invoke<AutomationRun>("automation_resume_run", { runId, input });
}

export function listAutomationRuns(
  workflowId?: string | null,
): Promise<AutomationRun[]> {
  return invoke<AutomationRun[]>("automation_list_runs", {
    workflowId: workflowId ?? null,
  });
}

export function getAutomationRun(runId: string): Promise<AutomationRunDetail | null> {
  return invoke<AutomationRunDetail | null>("automation_get_run", { runId });
}

export function onAutomationChanged(
  handler: (event: AutomationChangedEvent) => void,
): Promise<UnlistenFn> {
  return listen<AutomationChangedEvent>("automation:changed", (event) => {
    handler(event.payload);
  });
}

export function onAutomationRunUpdated(
  handler: (event: AutomationRunEvent) => void,
): Promise<UnlistenFn[]> {
  return Promise.all([
    listen<AutomationRunEvent>("automation:run-started", (event) => handler(event.payload)),
    listen<AutomationRunEvent>("automation:run-updated", (event) => handler(event.payload)),
    listen<AutomationRunEvent>("automation:run-finished", (event) => handler(event.payload)),
  ]);
}
