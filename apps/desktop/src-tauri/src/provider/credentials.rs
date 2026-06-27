use keyring::{Entry, Error as KeyringError};

use crate::{BACKEND_CLAUDE, BACKEND_CODEX};

const AI_CREDENTIAL_SERVICE: &str = "com.lilia.desktop.ai";
const ASSISTANT_AI_ACCOUNT: &str = "assistant-ai";

fn entry(account: &str) -> Result<Entry, String> {
    Entry::new(AI_CREDENTIAL_SERVICE, account).map_err(|e| format!("创建 AI 凭证项失败：{e}"))
}

pub(crate) fn provider_account(backend: &str) -> Result<String, String> {
    match backend {
        BACKEND_CLAUDE | BACKEND_CODEX => Ok(format!("provider:{backend}")),
        other => Err(format!("未知 backend: {other}")),
    }
}

pub(crate) fn assistant_ai_account() -> &'static str {
    ASSISTANT_AI_ACCOUNT
}

pub(crate) fn read_secret(account: &str) -> Result<Option<String>, String> {
    let entry = entry(account)?;
    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(err) => Err(format!("读取 AI 凭证失败：{err}")),
    }
}

pub(crate) fn write_secret(account: &str, secret: &str) -> Result<(), String> {
    let Some(trimmed) = normalize_secret(secret) else {
        return Ok(());
    };
    let entry = entry(account)?;
    entry
        .set_password(trimmed)
        .map_err(|e| format!("保存 AI 凭证失败：{e}"))
}

pub(crate) fn delete_secret(account: &str) -> Result<(), String> {
    let entry = entry(account)?;
    match entry.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(err) => Err(format!("删除 AI 凭证失败：{err}")),
    }
}

pub(crate) fn has_secret(account: &str) -> Result<bool, String> {
    Ok(read_secret(account)?
        .as_deref()
        .and_then(normalize_secret)
        .is_some())
}

pub(crate) fn normalize_secret(secret: &str) -> Option<&str> {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub(crate) fn apply_secret_update(
    account: &str,
    secret: Option<&str>,
    clear: bool,
) -> Result<(), String> {
    if clear {
        return delete_secret(account);
    }
    if let Some(secret) = secret.and_then(normalize_secret) {
        write_secret(account, secret)?;
    }
    Ok(())
}
