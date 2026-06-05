use tauri::{utils::config::Color, Manager, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

pub mod agent_events;
mod agent_extensions;
pub mod agent_timeline;
mod chat;
mod conversation_suggestions;
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
            chat::commands::chat_send_message,
            chat::commands::chat_interrupt_turn,
            chat::commands::chat_respond_agent_interaction,
            chat::commands::chat_list_models,
            chat::commands::chat_get_composer_state,
            chat::commands::chat_set_composer_state,
            chat::commands::chat_reset_session,
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
            popup_windows::popup_focus_main,
            provider::router_get_mode,
            provider::router_set_mode,
            conversation_suggestions::conversation_suggestions_get,
            conversation_suggestions::conversation_suggestions_get_settings,
            conversation_suggestions::conversation_suggestions_set_settings,
            project_shell::project_get_settings,
            project_shell::project_set_settings,
            project_shell::git_clone_repo,
            project_shell::system_open_path,
            project_shell::system_open_in_vscode,
            plugins::plugins_overview,
            plugins::plugins_list_claude_skills,
            plugins::plugins_create_claude_skill,
            plugins::plugins_delete_claude_skill,
            plugins::plugins_set_claude_skill_enabled,
            plugins::plugins_list_claude_plugins,
            plugins::plugins_set_claude_plugin_enabled,
            plugins::plugins_create_claude_mcp_server,
            plugins::plugins_update_claude_mcp_server,
            plugins::plugins_delete_claude_mcp_server,
            plugins::plugins_set_claude_mcp_server_enabled,
            plugins::plugins_open_claude_mcp_config,
            plugins::plugins_list_codex_mcp_servers,
            plugins::plugins_create_codex_mcp_server,
            plugins::plugins_update_codex_mcp_server,
            plugins::plugins_delete_codex_mcp_server,
            plugins::plugins_set_codex_mcp_server_enabled,
            plugins::plugins_open_codex_config,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
