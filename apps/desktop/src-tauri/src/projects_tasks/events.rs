use tauri::{AppHandle, Emitter, Runtime};

use crate::task_contract;

pub(crate) fn emit_tasks_changed<R: Runtime>(app: &AppHandle<R>, project_id: Option<String>) {
    if let Err(err) = app.emit(
        task_contract::tasks_changed_event_name(),
        task_contract::tasks_changed_event_payload(project_id.as_deref()),
    ) {
        eprintln!(
            "[tasks] emit {} failed: {err}",
            task_contract::tasks_changed_event_name()
        );
    }
}
