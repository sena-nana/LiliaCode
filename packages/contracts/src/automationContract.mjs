import automationContract from "./automation-contract.json" with { type: "json" };

const manifest = Object.freeze(automationContract);

export const AUTOMATION_CONTRACT = manifest;
export const AUTOMATION_TRIGGER_KINDS = manifest.automationTriggerKinds;
export const DEFAULT_AUTOMATION_TRIGGER_KIND = manifest.defaultAutomationTriggerKind;
export const AUTOMATION_SCOPE_EVENT_KINDS = manifest.automationScopeEventKinds;
export const AUTOMATION_SCOPE_TASK_STATUSES = manifest.automationScopeTaskStatuses;
export const AUTOMATION_TRIGGER_KIND_LABELS = manifest.automationTriggerKindLabels;
export const AUTOMATION_LOGIC_KINDS = manifest.automationLogicKinds;
export const DEFAULT_AUTOMATION_LOGIC_KIND = manifest.defaultAutomationLogicKind;
export const DEFAULT_AUTOMATION_LOGIC_PATH = manifest.defaultAutomationLogicPath;
export const AUTOMATION_LOGIC_KIND_LABELS = manifest.automationLogicKindLabels;
export const DEFAULT_AUTOMATION_AGENT_PROMPT = manifest.defaultAutomationAgentPrompt;
export const DEFAULT_AUTOMATION_HUMAN_PROMPT = manifest.defaultAutomationHumanPrompt;
export const AUTOMATION_RUN_STATUSES = manifest.automationRunStatuses;
export const AUTOMATION_CHANGED_EVENT_NAME = manifest.automationChangedEventName;
export const AUTOMATION_RUN_STARTED_EVENT_NAME = manifest.automationRunStartedEventName;
export const AUTOMATION_RUN_UPDATED_EVENT_NAME = manifest.automationRunUpdatedEventName;
export const AUTOMATION_RUN_FINISHED_EVENT_NAME = manifest.automationRunFinishedEventName;
export const AUTOMATION_LIST_WORKFLOWS_COMMAND =
  manifest.automationListWorkflowsCommand;
export const AUTOMATION_SAVE_DRAFT_COMMAND = manifest.automationSaveDraftCommand;
export const AUTOMATION_PUBLISH_COMMAND = manifest.automationPublishCommand;
export const AUTOMATION_DELETE_WORKFLOW_COMMAND =
  manifest.automationDeleteWorkflowCommand;
export const AUTOMATION_SET_ENABLED_COMMAND = manifest.automationSetEnabledCommand;
export const AUTOMATION_RUN_ONCE_COMMAND = manifest.automationRunOnceCommand;
export const AUTOMATION_RESUME_RUN_COMMAND = manifest.automationResumeRunCommand;
export const AUTOMATION_LIST_RUNS_COMMAND = manifest.automationListRunsCommand;
export const AUTOMATION_GET_RUN_COMMAND = manifest.automationGetRunCommand;
export const AUTOMATION_RUN_EVENT_NAMES = Object.freeze([
  AUTOMATION_RUN_STARTED_EVENT_NAME,
  AUTOMATION_RUN_UPDATED_EVENT_NAME,
  AUTOMATION_RUN_FINISHED_EVENT_NAME,
]);
export const DEFAULT_AUTOMATION_RUN_STATUS = manifest.defaultAutomationRunStatus;
export const AUTOMATION_WAITING_USER_STATUS = manifest.automationWaitingUserStatus;
export const AUTOMATION_RUN_STATUS_TONES = manifest.automationRunStatusTones;
export const AUTOMATION_TOOL_ACTIONS = manifest.automationToolActions;
export const DEFAULT_AUTOMATION_TOOL_ACTION = manifest.defaultAutomationToolAction;
export const AUTOMATION_TOOL_ACTION_LABELS = manifest.automationToolActionLabels;
export const AUTOMATION_TOOL_ACTION_FIELDS = manifest.automationToolActionFields;
export const AUTOMATION_TOOL_PRIORITIES = manifest.automationToolPriorities;
export const DEFAULT_AUTOMATION_TOOL_PRIORITY =
  manifest.defaultAutomationToolPriority;
