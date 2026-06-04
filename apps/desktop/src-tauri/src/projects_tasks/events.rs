use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TasksChangedEvent {
    project_id: Option<String>,
}

pub(super) fn emit_tasks_changed(app: &AppHandle, project_id: Option<String>) {
    if let Err(err) = app.emit("tasks:changed", TasksChangedEvent { project_id }) {
        eprintln!("[tasks] emit tasks:changed failed: {err}");
    }
}
