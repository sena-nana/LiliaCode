import { invoke } from "../tauri/runtime";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AutomationRun,
  AutomationRunNodeState,
  AutomationRunOnceInput,
  AutomationResumeRunInput,
  AutomationSaveDraftInput,
  AutomationRunSummary,
  AutomationChangedEvent,
  AutomationRunEvent,
  AutomationWorkflow,
  AutomationWorkflowVersion,
} from "@lilia/contracts";
import {
  AUTOMATION_CHANGED_EVENT_NAME,
  AUTOMATION_DELETE_WORKFLOW_COMMAND,
  AUTOMATION_GET_RUN_COMMAND,
  AUTOMATION_LIST_RUNS_COMMAND,
  AUTOMATION_LIST_WORKFLOWS_COMMAND,
  AUTOMATION_PUBLISH_COMMAND,
  AUTOMATION_RESUME_RUN_COMMAND,
  AUTOMATION_RUN_ONCE_COMMAND,
  AUTOMATION_RUN_EVENT_NAMES,
  AUTOMATION_SAVE_DRAFT_COMMAND,
  AUTOMATION_SET_ENABLED_COMMAND,
} from "@lilia/contracts";
import { installUnlistenFns } from "../utils/eventListeners";

export type {
  AutomationRun,
  AutomationRunNodeState,
  AutomationRunOnceInput,
  AutomationResumeRunInput,
  AutomationSaveDraftInput,
  AutomationRunSummary,
  AutomationChangedEvent,
  AutomationRunEvent,
  AutomationWorkflow,
  AutomationWorkflowVersion,
};

export interface AutomationRunDetail {
  run: AutomationRun;
  nodes: AutomationRunNodeState[];
}

export function listAutomations(): Promise<AutomationWorkflow[]> {
  return invoke<AutomationWorkflow[]>(AUTOMATION_LIST_WORKFLOWS_COMMAND);
}

export function saveAutomationDraft(
  input: AutomationSaveDraftInput,
): Promise<AutomationWorkflow> {
  return invoke<AutomationWorkflow>(AUTOMATION_SAVE_DRAFT_COMMAND, { input });
}

export function publishAutomation(id: string): Promise<AutomationWorkflowVersion> {
  return invoke<AutomationWorkflowVersion>(AUTOMATION_PUBLISH_COMMAND, { id });
}

export function deleteAutomation(id: string): Promise<void> {
  return invoke<void>(AUTOMATION_DELETE_WORKFLOW_COMMAND, { id });
}

export function setAutomationEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke<void>(AUTOMATION_SET_ENABLED_COMMAND, { id, enabled });
}

export function runAutomationOnce(
  id: string,
  input: AutomationRunOnceInput = {},
): Promise<AutomationRun> {
  return invoke<AutomationRun>(AUTOMATION_RUN_ONCE_COMMAND, { id, input });
}

export function resumeAutomationRun(
  runId: string,
  input: AutomationResumeRunInput = {},
): Promise<AutomationRun> {
  return invoke<AutomationRun>(AUTOMATION_RESUME_RUN_COMMAND, { runId, input });
}

export function listAutomationRuns(
  workflowId?: string | null,
): Promise<AutomationRunSummary[]> {
  return invoke<AutomationRunSummary[]>(AUTOMATION_LIST_RUNS_COMMAND, {
    workflowId: workflowId ?? null,
  });
}

export function getAutomationRun(runId: string): Promise<AutomationRunDetail | null> {
  return invoke<AutomationRunDetail | null>(AUTOMATION_GET_RUN_COMMAND, { runId });
}

export function onAutomationChanged(
  handler: (event: AutomationChangedEvent) => void,
): Promise<UnlistenFn> {
  return listen<AutomationChangedEvent>(AUTOMATION_CHANGED_EVENT_NAME, (event) => {
    handler(event.payload);
  });
}

export function onAutomationRunUpdated(
  handler: (event: AutomationRunEvent) => void,
): Promise<UnlistenFn[]> {
  return installUnlistenFns(
    AUTOMATION_RUN_EVENT_NAMES.map((eventName) => () =>
      listen<AutomationRunEvent>(eventName, (event) => handler(event.payload))
    ),
  );
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

