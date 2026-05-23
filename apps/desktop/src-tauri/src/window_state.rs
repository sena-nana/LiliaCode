use serde::{Deserialize, Serialize};
use tauri::{AppHandle, PhysicalPosition, PhysicalSize, WebviewWindow};
use tauri_plugin_store::StoreExt;

pub const MAIN_WINDOW_STATE_STORE_FILE: &str = "main-window-state.json";
pub const MAIN_WINDOW_STATE_KEY: &str = "mainWindow";
pub const MIN_MAIN_WINDOW_WIDTH: u32 = 960;
pub const MIN_MAIN_WINDOW_HEIGHT: u32 = 600;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MainWindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MainWindowSnapshot {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

impl MainWindowSnapshot {
    fn into_state(self) -> MainWindowState {
        MainWindowState {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            maximized: self.maximized,
        }
    }
}

pub fn is_restorable_main_window_state(state: &MainWindowState) -> bool {
    state.width >= MIN_MAIN_WINDOW_WIDTH && state.height >= MIN_MAIN_WINDOW_HEIGHT
}

pub fn merge_main_window_state(
    previous: Option<MainWindowState>,
    snapshot: MainWindowSnapshot,
) -> MainWindowState {
    if snapshot.maximized {
        if let Some(previous) = previous.filter(is_restorable_main_window_state) {
            return MainWindowState {
                maximized: true,
                ..previous
            };
        }
    }
    snapshot.into_state()
}

pub fn load_main_window_state(app: &AppHandle) -> Option<MainWindowState> {
    let store = app.store(MAIN_WINDOW_STATE_STORE_FILE).ok()?;
    let value = store.get(MAIN_WINDOW_STATE_KEY)?;
    serde_json::from_value::<MainWindowState>(value)
        .ok()
        .filter(is_restorable_main_window_state)
}

pub fn save_main_window_state(app: &AppHandle, snapshot: MainWindowSnapshot) -> Result<(), String> {
    let store = app
        .store(MAIN_WINDOW_STATE_STORE_FILE)
        .map_err(|e| format!("打开窗口状态存储失败：{e}"))?;
    let previous = store
        .get(MAIN_WINDOW_STATE_KEY)
        .and_then(|value| serde_json::from_value::<MainWindowState>(value).ok());
    let state = merge_main_window_state(previous, snapshot);
    let value = serde_json::to_value(state).map_err(|e| e.to_string())?;
    store.set(MAIN_WINDOW_STATE_KEY, value);
    store.save().map_err(|e| format!("保存窗口状态失败：{e}"))
}

pub fn capture_main_window_snapshot(window: &WebviewWindow) -> Option<MainWindowSnapshot> {
    let position = window.outer_position().ok()?;
    let size = window.inner_size().ok()?;
    let maximized = window.is_maximized().unwrap_or(false);
    Some(MainWindowSnapshot {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized,
    })
}

pub fn restore_main_window_state(window: &WebviewWindow, state: MainWindowState) {
    let _ = window.set_position(PhysicalPosition::new(state.x, state.y));
    let _ = window.set_size(PhysicalSize::new(state.width, state.height));
    if state.maximized {
        let _ = window.maximize();
    }
}

pub fn persist_main_window_state(app: &AppHandle, window: &WebviewWindow) {
    let Some(snapshot) = capture_main_window_snapshot(window) else {
        return;
    };
    if let Err(err) = save_main_window_state(app, snapshot) {
        eprintln!("[window-state] {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maximized_snapshot_keeps_last_normal_geometry() {
        let previous = MainWindowState {
            x: 120,
            y: 80,
            width: 1180,
            height: 760,
            maximized: false,
        };
        let maximized_snapshot = MainWindowSnapshot {
            x: -8,
            y: -8,
            width: 1936,
            height: 1056,
            maximized: true,
        };

        let merged = merge_main_window_state(Some(previous), maximized_snapshot);

        assert_eq!(
            merged,
            MainWindowState {
                maximized: true,
                ..previous
            }
        );
    }

    #[test]
    fn main_window_starts_hidden_until_restored() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let main_window = config["app"]["windows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|window| window["label"].as_str() == Some("main"))
            .unwrap();

        assert_eq!(main_window["visible"].as_bool(), Some(false));
    }
}
