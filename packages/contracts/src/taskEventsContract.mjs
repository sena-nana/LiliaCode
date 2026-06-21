import taskEventsContract from "./task-events-contract.json" with { type: "json" };

const manifest = Object.freeze(taskEventsContract);

export const TASK_EVENTS_CONTRACT = manifest;
export const TASKS_CHANGED_EVENT_NAME = manifest.tasksChangedEventName;
