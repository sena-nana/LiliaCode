import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AutomationRun,
  AutomationRunNodeState,
  AutomationRunOnceInput,
  AutomationResumeRunInput,
  AutomationSaveDraftInput,
  AutomationRunSummary,
  AutomationWorkflow,
  AutomationWorkflowVersion,
} from "@lilia/contracts";

export type {
  AutomationRun,
  AutomationRunNodeState,
  AutomationRunOnceInput,
  AutomationResumeRunInput,
  AutomationSaveDraftInput,
  AutomationRunSummary,
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

export function deleteAutomation(id: string): Promise<void> {
  return invoke<void>("automation_delete_workflow", { id });
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
): Promise<AutomationRunSummary[]> {
  return invoke<AutomationRunSummary[]>("automation_list_runs", {
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

export function automationRunToSummary(run: AutomationRun): AutomationRunSummary {
  return {
    id: run.id,
    workflowId: run.workflowId,
    workflowVersionId: run.workflowVersionId,
    status: run.status,
    triggerKind: run.trigger.kind,
    projectId: run.trigger.projectId ?? null,
    taskId: run.trigger.taskId ?? null,
    backend: run.trigger.backend ?? null,
    eventKind: run.trigger.eventKind ?? null,
    startedAt: run.startedAt,
    finishedAt: run.finishedAt,
    error: run.error,
  };
}
