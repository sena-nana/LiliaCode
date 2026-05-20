use tauri::{
    utils::config::Color, Manager, Theme, WebviewWindow, WindowEvent,
};

const MAIN_WINDOW_LABEL: &str = "main";

// 与前端 CSS 变量保持一致：light=#f4f5f7、dark=#1c1c1e。
// 把窗口背景色刷子调成这两个色，避免 Windows 拉伸时露出白底。
const BG_LIGHT: Color = Color(0xF4, 0xF5, 0xF7, 0xFF);
const BG_DARK: Color = Color(0x1C, 0x1C, 0x1E, 0xFF);

fn background_for(theme: Theme) -> Color {
    match theme {
        Theme::Dark => BG_DARK,
        _ => BG_LIGHT,
    }
}

fn apply_background(window: &WebviewWindow, theme: Theme) {
    let _ = window.set_background_color(Some(background_for(theme)));
}

#[tauri::command]
fn ping() -> &'static str {
    "pong"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                let initial_theme = window.theme().unwrap_or(Theme::Light);
                apply_background(&window, initial_theme);
                let follow = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::ThemeChanged(theme) = event {
                        apply_background(&follow, *theme);
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![ping])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
