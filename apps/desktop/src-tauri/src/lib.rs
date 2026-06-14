use tauri::{utils::config::Color, Manager, Runtime, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

pub mod agent_events;
mod agent_extensions;
pub mod agent_timeline;
mod automation;
mod chat;
mod claude_history;
mod codex_history;
mod conversation_suggestions;
mod github;
mod history_import;
mod lilia_iab;
mod plugins;
mod popup_windows;
mod project_shell;
mod projects_tasks;
mod provider;
mod settings_store;
mod store;
mod todos;
mod tray;
mod util;
mod window_state;

pub(crate) const MAIN_WINDOW_LABEL: &str = "main";

// 始终使用暗色：与前端 CSS 变量 --bg = #181818 保持一致，避免 Windows 拉伸/还原时
// 露出 WebView 之外的默认白底。
pub(crate) const BG: Color = Color(0x18, 0x18, 0x18, 0xFF);

pub(crate) const BACKEND_CLAUDE: &str = "claude";
pub(crate) const BACKEND_CODEX: &str = "codex";
pub(crate) const RUNTIME_CHANNEL_BUILTIN: &str = "builtin";
pub(crate) const RUNTIME_CHANNEL_MUTSUKI_CORE: &str = "mutsuki_core";
pub(crate) const CODEX_MODEL_OPTIONS: [(&str, &str); 3] = [
    ("gpt-5.5", "GPT-5.5"),
    ("gpt-5.4", "GPT-5.4"),
    ("gpt-5.4-mini", "GPT-5.4 Mini"),
];
pub(crate) const MIN_CODEX_APP_SERVER_VERSION: (u32, u32, u32) = (0, 128, 0);

#[tauri::command]
fn ping() -> &'static str {
    "pong"
}

fn handle_runtime_restore_failure<R: Runtime>(
    app: &tauri::AppHandle<R>,
    persisted: &chat::state::PersistedRuntimeState,
    runtime_name: &str,
    err: String,
) {
    let store = app.state::<chat::state::ChatStore>();
    store
        .running_tasks
        .lock()
        .unwrap()
        .remove(&persisted.task_id);
    chat::state::clear_running_handles(&store, &persisted.task_id);
    chat::state::clear_runtime_state_for_app(app, &persisted.task_id);
    chat::timeline_sink::persist_and_emit_error_timeline_event(
        app,
        &persisted.task_id,
        &persisted.turn.backend,
        Some(&persisted.turn.turn_id),
        format!("恢复 {runtime_name} runtime 失败：{err}"),
    );
}

fn restore_runtime_sessions_on_startup<R: Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(chat_store) = app.try_state::<chat::state::ChatStore>() {
        if let Some(lilia_store) = app.try_state::<store::LiliaStore>() {
            if let Ok(conn) = lilia_store.conn() {
                let restored = chat::state::restore_active_runtime_sessions(&conn, &chat_store);
                for persisted in restored {
                    let app_handle = app.clone();
                    std::thread::spawn(move || match persisted.turn.runtime_channel.as_str() {
                        RUNTIME_CHANNEL_MUTSUKI_CORE => {
                            if let Err(err) = chat::mutsuki_core_runtime::resume_supervised_turn(
                                app_handle.clone(),
                                persisted.clone(),
                            ) {
                                handle_runtime_restore_failure(
                                    &app_handle,
                                    &persisted,
                                    "MutsukiCore",
                                    err,
                                );
                            }
                        }
                        _ => {
                            if let Err(err) = chat::runner::resume_persisted_node_agent_runner(
                                app_handle.clone(),
                                persisted.clone(),
                            ) {
                                handle_runtime_restore_failure(
                                    &app_handle,
                                    &persisted,
                                    "builtin",
                                    err,
                                );
                            }
                        }
                    });
                }
                if let Err(err) = automation::recover_abandoned_agent_runs(app, &chat_store) {
                    eprintln!("[automation] recover abandoned agent runs failed: {err}");
                }
                if let Ok(task_ids) = chat::state::list_pending_turn_task_ids(&conn) {
                    for task_id in task_ids {
                        let app_handle = app.clone();
                        std::thread::spawn(move || {
                            if let Err(err) =
                                chat::runner::resume_or_dispatch_persisted_pending_turn(
                                    app_handle.clone(),
                                    task_id.clone(),
                                )
                            {
                                eprintln!(
                                    "[chat-runtime] restore persisted queued turn failed for {}: {}",
                                    task_id, err
                                );
                            }
                        });
                    }
                }
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if let Err(err) = popup_windows::open_popup_for_shortcut(app) {
                            eprintln!("[popup-window] shortcut failed: {err}");
                        }
                    }
                })
                .build(),
        )
        .manage(chat::state::ChatStore::default())
        .setup(|app| {
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                let _ = window.set_background_color(Some(BG));
                if let Some(state) = window_state::load_main_window_state(app.handle()) {
                    window_state::restore_main_window_state(&window, state);
                }
                let _ = window.show();
            }
            let home = store::resolve_lilia_home();
            match store::LiliaStore::new(&home) {
                Ok(s) => {
                    app.manage(s);
                    restore_runtime_sessions_on_startup(app.handle());
                }
                Err(err) => {
                    eprintln!("[lilia-store] init failed at {}: {err}", home.display());
                }
            }
            popup_windows::register_initial_popup_shortcut(app.handle());
            if let Err(err) = tray::setup_tray(app.handle()) {
                eprintln!("[tray] setup failed: {err}");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }
            if matches!(
                event,
                WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed
            ) {
                if let Some(webview_window) = window.get_webview_window(MAIN_WINDOW_LABEL) {
                    window_state::persist_main_window_state(&window.app_handle(), &webview_window);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            chat::attachments::chat_describe_attachments,
            chat::attachments::chat_read_clipboard_file_paths,
            chat::attachments::chat_save_clipboard_image,
            chat::attachments::chat_save_clipboard_text,
            chat::attachments::chat_search_context_attachments,
            chat::slash_commands::chat_search_slash_commands,
            chat::commands::chat_send_message,
            chat::commands::chat_interrupt_turn,
            chat::commands::chat_respond_agent_interaction,
            chat::title_update::chat_respond_title_update,
            chat::commands::chat_list_models,
            chat::commands::chat_get_composer_state,
            chat::commands::chat_get_runtime_snapshot,
            chat::commands::chat_ack_restored_rollback,
            chat::commands::chat_set_composer_state,
            chat::commands::chat_reset_session,
            history_import::history_import_search,
            history_import::history_import_preview,
            history_import::history_import_attach,
            history_import::history_import_runtime_states,
            history_import::history_import_clean_background_terminals,
            lilia_iab::lilia_iab_open,
            lilia_iab::lilia_iab_submit,
            provider::chat_check_env,
            provider::provider_get_config,
            provider::provider_set_config,
            provider::provider_get_active_backend,
            provider::provider_set_active_backend,
            provider::cc_switch_get_config,
            provider::cc_switch_set_config,
            provider::assistant_ai_get_config,
            provider::assistant_ai_set_config,
            provider::assistant_ai_test_connection,
            provider::agent_interaction_get_settings,
            provider::agent_interaction_set_settings,
            popup_windows::popup_get_window_settings,
            popup_windows::popup_set_window_settings,
            popup_windows::popup_remember_last_project,
            popup_windows::popup_open_new_chat,
            popup_windows::popup_open_task,
            popup_windows::popup_open_child_question,
            popup_windows::popup_open_conversation_status,
            popup_windows::popup_toggle_conversation_status,
            popup_windows::popup_focus_main,
            provider::router_get_mode,
            provider::router_set_mode,
            conversation_suggestions::conversation_suggestions_get,
            conversation_suggestions::conversation_suggestions_get_settings,
            conversation_suggestions::conversation_suggestions_set_settings,
            project_shell::project_get_settings,
            project_shell::project_set_settings,
            project_shell::git_clone_repo,
            github::github_get_binding_status,
            github::github_start_device_flow,
            github::github_poll_device_flow,
            github::github_unbind,
            github::github_list_repos,
            github::github_clone_repo,
            project_shell::system_open_path,
            project_shell::system_open_url,
            project_shell::system_open_in_vscode,
            plugins::plugins_overview,
            plugins::plugins_list_skills,
            plugins::plugins_create_skill,
            plugins::plugins_delete_skill,
            plugins::plugins_set_skill_enabled,
            plugins::plugins_list_packages,
            plugins::plugins_set_package_enabled,
            plugins::plugins_create_mcp_server,
            plugins::plugins_update_mcp_server,
            plugins::plugins_delete_mcp_server,
            plugins::plugins_set_mcp_server_enabled,
            plugins::plugins_open_mcp_config,
            todos::todo_list,
            todos::todo_create,
            todos::todo_update,
            todos::todo_delete,
            todos::todo_apply_agent_event,
            projects_tasks::project_list,
            projects_tasks::project_get,
            projects_tasks::project_create,
            projects_tasks::project_rename,
            projects_tasks::project_remove,
            projects_tasks::project_toggle_pin,
            projects_tasks::milestone_list,
            projects_tasks::milestone_create,
            projects_tasks::milestone_update,
            projects_tasks::milestone_set_tasks,
            projects_tasks::project_architecture_get,
            projects_tasks::project_architecture_list_changes,
            projects_tasks::project_architecture_apply,
            projects_tasks::project_architecture_reject,
            projects_tasks::project_architecture_rollback,
            projects_tasks::task_list,
            projects_tasks::task_get,
            projects_tasks::task_create,
            projects_tasks::task_update,
            projects_tasks::task_delete,
            projects_tasks::task_promote,
            projects_tasks::task_archive_project,
            projects_tasks::task_archive,
            projects_tasks::task_toggle_pin,
            projects_tasks::project_reorder,
            projects_tasks::task_reorder,
            projects_tasks::task_reparent,
            agent_timeline::agent_timeline_list,
            agent_timeline::agent_timeline_clear_task,
            automation::commands::automation_list_workflows,
            automation::commands::automation_get_workflow,
            automation::commands::automation_save_draft,
            automation::commands::automation_publish,
            automation::commands::automation_set_enabled,
            automation::commands::automation_run_once,
            automation::commands::automation_resume_run,
            automation::commands::automation_list_runs,
            automation::commands::automation_get_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
