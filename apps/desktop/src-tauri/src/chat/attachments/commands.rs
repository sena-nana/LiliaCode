use crate::chat::attachments::clipboard::{
    read_clipboard_file_paths, save_clipboard_image, save_clipboard_text,
};
use crate::chat::attachments::context_search::search_context_attachments;
use crate::chat::attachments::describe::describe_attachment_path;
use crate::chat::types::{
    ChatAttachment, ChatContextSearchResult, ClipboardImageInput, ClipboardTextInput,
};

#[tauri::command]
pub fn chat_describe_attachments(paths: Vec<String>) -> Result<Vec<ChatAttachment>, String> {
    Ok(paths
        .into_iter()
        .filter(|path| !path.trim().is_empty())
        .map(describe_attachment_path)
        .collect())
}

#[tauri::command]
pub fn chat_read_clipboard_file_paths() -> Result<Vec<String>, String> {
    read_clipboard_file_paths()
}

#[tauri::command]
pub fn chat_save_clipboard_image(input: ClipboardImageInput) -> Result<ChatAttachment, String> {
    save_clipboard_image(input)
}

#[tauri::command]
pub fn chat_save_clipboard_text(input: ClipboardTextInput) -> Result<ChatAttachment, String> {
    save_clipboard_text(input)
}

#[tauri::command]
pub fn chat_search_context_attachments(
    project_cwd: String,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<ChatContextSearchResult>, String> {
    search_context_attachments(project_cwd, query, limit)
}
