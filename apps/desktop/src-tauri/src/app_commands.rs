use tauri::{AppHandle, Runtime};

#[tauri::command]
pub(crate) fn app_restart<R: Runtime>(app: AppHandle<R>) {
    app.restart();
}
