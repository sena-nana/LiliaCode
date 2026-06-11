use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TasksChangedEvent {
    project_id: Option<String>,
}

pub(crate) fn emit_tasks_changed<R: Runtime>(app: &AppHandle<R>, project_id: Option<String>) {
    if let Err(err) = app.emit("tasks:changed", TasksChangedEvent { project_id }) {
        eprintln!("[tasks] emit tasks:changed failed: {err}");
    }
}
