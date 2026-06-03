use serde::{de::DeserializeOwned, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

pub(crate) const PROVIDER_STORE_FILE: &str = "provider-config.json";

pub(crate) fn load_store_value<T: DeserializeOwned>(app: &AppHandle, key: &str) -> Option<T> {
    let store = app.store(PROVIDER_STORE_FILE).ok()?;
    let value = store.get(key)?;
    serde_json::from_value::<T>(value).ok()
}

pub(crate) fn save_store_value<T: Serialize>(
    app: &AppHandle,
    key: &str,
    value: &T,
) -> Result<(), String> {
    let store = app
        .store(PROVIDER_STORE_FILE)
        .map_err(|e| format!("打开配置存储失败：{e}"))?;
    let value = serde_json::to_value(value).map_err(|e| e.to_string())?;
    store.set(key, value);
    store.save().map_err(|e| format!("保存配置失败：{e}"))?;
    Ok(())
}

pub(crate) fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
