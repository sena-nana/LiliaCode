use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use base64::{engine::general_purpose, Engine as _};
use ignore::WalkBuilder;
use uuid::Uuid;

use crate::chat::state::now_millis;
use crate::chat::types::{
    ChatAttachment, ChatAttachmentDirectoryMeta, ChatContextSearchResult, ClipboardImageInput,
    ClipboardTextInput,
};
use crate::store;

static CLIPBOARD_IMAGE_DISPLAY_SEQ: AtomicU64 = AtomicU64::new(0);
static CLIPBOARD_TEXT_DISPLAY_SEQ: AtomicU64 = AtomicU64::new(0);

const DIRECTORY_SCAN_LIMIT: usize = 1000;
const CONTEXT_SEARCH_SCAN_LIMIT: usize = 6000;
const DEFAULT_CONTEXT_SEARCH_LIMIT: usize = 12;
const MAX_CONTEXT_SEARCH_LIMIT: usize = 50;

fn image_mime_for_path(path: &Path) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())?;
    let mime = match ext.as_str() {
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => return None,
    };
    Some(mime.to_string())
}

fn clipboard_image_extension(mime: Option<&str>, name: Option<&str>) -> &'static str {
    let normalized = mime.unwrap_or("").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "image/avif" => "avif",
        "image/bmp" => "bmp",
        "image/gif" => "gif",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/svg+xml" => "svg",
        "image/webp" => "webp",
        _ => name
            .and_then(|value| Path::new(value).extension())
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .and_then(|ext| match ext.as_str() {
                "avif" => Some("avif"),
                "bmp" => Some("bmp"),
                "gif" => Some("gif"),
                "jpg" | "jpeg" => Some("jpg"),
                "png" => Some("png"),
                "svg" => Some("svg"),
                "webp" => Some("webp"),
                _ => None,
            })
            .unwrap_or("png"),
    }
}

fn normalize_clipboard_image_mime(mime: Option<&str>, ext: &str) -> String {
    let normalized = mime.unwrap_or("").trim().to_ascii_lowercase();
    if normalized.starts_with("image/") {
        return normalized;
    }
    match ext {
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => "image/png",
    }
    .to_string()
}

fn clipboard_images_cache_dir(home: &Path) -> PathBuf {
    home.join("cache").join("clipboard-images")
}

fn clipboard_image_path(home: &Path, mime: Option<&str>, name: Option<&str>, now: u64) -> PathBuf {
    let ext = clipboard_image_extension(mime, name);
    clipboard_images_cache_dir(home).join(format!("clipboard-{now}-{}.{}", Uuid::new_v4(), ext))
}

fn clipboard_image_display_name(ext: &str, seq: u64) -> String {
    format!("图片 {seq}.{ext}")
}

fn clipboard_texts_cache_dir(home: &Path) -> PathBuf {
    home.join("cache").join("clipboard-texts")
}

fn clipboard_text_path(home: &Path, now: u64) -> PathBuf {
    clipboard_texts_cache_dir(home).join(format!("clipboard-{now}-{}.txt", Uuid::new_v4()))
}

fn clipboard_text_display_name(seq: u64) -> String {
    format!("粘贴文本 {seq}.txt")
}

fn save_clipboard_image_to_cache(
    home: &Path,
    input: ClipboardImageInput,
    now: u64,
    display_seq: u64,
) -> Result<ChatAttachment, String> {
    let path = clipboard_image_path(home, input.mime.as_deref(), input.name.as_deref(), now);
    let parent = path
        .parent()
        .ok_or_else(|| "无法解析剪贴板图片缓存目录".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("创建剪贴板图片缓存目录失败：{e}"))?;
    let bytes = general_purpose::STANDARD
        .decode(input.bytes_base64.trim())
        .map_err(|e| format!("解析剪贴板图片失败：{e}"))?;
    fs::write(&path, bytes).map_err(|e| format!("保存剪贴板图片失败：{e}"))?;
    let mut attachment = describe_attachment_path(path.to_string_lossy().to_string());
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    attachment.name = clipboard_image_display_name(&ext, display_seq);
    attachment.mime = Some(normalize_clipboard_image_mime(input.mime.as_deref(), &ext));
    Ok(attachment)
}

fn save_clipboard_text_to_cache(
    home: &Path,
    input: ClipboardTextInput,
    now: u64,
    display_seq: u64,
) -> Result<ChatAttachment, String> {
    let path = clipboard_text_path(home, now);
    let parent = path
        .parent()
        .ok_or_else(|| "无法解析剪贴板文本缓存目录".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("创建剪贴板文本缓存目录失败：{e}"))?;
    fs::write(&path, input.text.as_bytes()).map_err(|e| format!("保存剪贴板文本失败：{e}"))?;
    let mut attachment = describe_attachment_path(path.to_string_lossy().to_string());
    attachment.name = clipboard_text_display_name(display_seq);
    attachment.mime = None;
    Ok(attachment)
}

fn scan_directory_meta(path: &Path) -> ChatAttachmentDirectoryMeta {
    let mut meta = ChatAttachmentDirectoryMeta {
        file_count: 0,
        directory_count: 0,
        total_size: 0,
        truncated: false,
        unreadable_count: 0,
    };
    let mut scanned = 0usize;
    let mut queue = VecDeque::from([path.to_path_buf()]);

    'scan: while let Some(dir) = queue.pop_front() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => {
                meta.unreadable_count = meta.unreadable_count.saturating_add(1);
                continue;
            }
        };
        for entry in entries {
            if scanned >= DIRECTORY_SCAN_LIMIT {
                meta.truncated = true;
                break 'scan;
            }
            scanned += 1;
            let Ok(entry) = entry else {
                meta.unreadable_count = meta.unreadable_count.saturating_add(1);
                continue;
            };
            let Ok(file_type) = entry.file_type() else {
                meta.unreadable_count = meta.unreadable_count.saturating_add(1);
                continue;
            };
            if file_type.is_dir() {
                meta.directory_count = meta.directory_count.saturating_add(1);
                queue.push_back(entry.path());
            } else if file_type.is_file() {
                meta.file_count = meta.file_count.saturating_add(1);
                if let Ok(file_meta) = entry.metadata() {
                    meta.total_size = meta.total_size.saturating_add(file_meta.len());
                } else {
                    meta.unreadable_count = meta.unreadable_count.saturating_add(1);
                }
            }
        }
    }

    meta
}

fn describe_attachment_path(path: String) -> ChatAttachment {
    let raw_path = PathBuf::from(path.trim());
    let normalized_path = if raw_path.is_absolute() {
        raw_path
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(&raw_path))
            .unwrap_or(raw_path)
    };
    let metadata = fs::metadata(&normalized_path).ok();
    let kind = metadata
        .as_ref()
        .map(|meta| {
            if meta.is_file() {
                "file"
            } else if meta.is_dir() {
                "directory"
            } else {
                "unknown"
            }
        })
        .unwrap_or("unknown")
        .to_string();
    let exists = metadata.is_some();
    let size = metadata.as_ref().and_then(|meta| {
        if meta.is_file() {
            Some(meta.len())
        } else {
            None
        }
    });
    let mime = if kind == "file" {
        image_mime_for_path(&normalized_path)
    } else {
        None
    };
    let directory = metadata.as_ref().and_then(|meta| {
        if meta.is_dir() {
            Some(scan_directory_meta(&normalized_path))
        } else {
            None
        }
    });
    let path_text = normalized_path.to_string_lossy().to_string();
    let name = normalized_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path_text.clone());

    ChatAttachment {
        id: format!("att-{}", Uuid::new_v4()),
        name,
        path: path_text,
        kind,
        size,
        exists,
        mime,
        directory,
    }
}

fn should_skip_context_search_dir(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_default();
    if matches!(
        name.as_str(),
        ".git" | "node_modules" | "dist" | "target" | ".cache" | "build"
    ) {
        return true;
    }
    if name == "cache" {
        return path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|parent| parent.to_str())
            .map(|parent| parent.eq_ignore_ascii_case(".yarn"))
            .unwrap_or(false);
    }
    false
}

fn relative_path_text(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn sorted_child_paths(dir: &Path) -> Result<Vec<PathBuf>, ()> {
    let mut paths = Vec::new();
    let entries = fs::read_dir(dir).map_err(|_| ())?;
    for entry in entries {
        if let Ok(entry) = entry {
            paths.push(entry.path());
        }
    }
    paths.sort_by_key(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_ascii_lowercase())
            .unwrap_or_default()
    });
    Ok(paths)
}

fn context_query_is_path_like(query: &str) -> bool {
    query.contains('/') || query.contains('\\')
}

fn context_query_allows_hidden(query: &str) -> bool {
    query.contains('.')
}

fn is_hidden_context_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with('.'))
        .unwrap_or(false)
}

fn normalize_context_path_query(query: &str) -> String {
    let mut normalized = query.trim().replace('\\', "/");
    while let Some(rest) = normalized.strip_prefix("./") {
        normalized = rest.to_string();
    }
    normalized
}

fn context_relative_path_buf(path_text: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    if path_text.trim().is_empty() {
        return Some(path);
    }
    for component in Path::new(path_text).components() {
        match component {
            Component::Normal(part) => path.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    Some(path)
}

fn context_browse_dir_from_query(query: &str) -> Option<(PathBuf, String)> {
    let normalized = normalize_context_path_query(query);
    if !context_query_is_path_like(query) {
        return None;
    }
    if normalized.is_empty() {
        return Some((PathBuf::new(), normalized));
    }
    if normalized.ends_with('/') {
        let dir_text = normalized.trim_end_matches('/');
        return context_relative_path_buf(dir_text).map(|dir| (dir, normalized));
    }
    let slash = normalized.rfind('/')?;
    let dir_text = &normalized[..slash];
    context_relative_path_buf(dir_text).map(|dir| (dir, normalized))
}

fn context_search_match(root: &Path, path: &Path, query: &str) -> Option<(String, String)> {
    let relative_path = relative_path_text(root, path);
    if query.is_empty() {
        return Some((relative_path, "name".to_string()));
    }
    let query = query.to_ascii_lowercase().replace('\\', "/");
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.contains(&query) {
        return Some((relative_path, "name".to_string()));
    }
    if relative_path.to_ascii_lowercase().contains(&query) {
        return Some((relative_path, "path".to_string()));
    }
    None
}

fn push_context_search_result(
    root: &Path,
    path: &Path,
    query: &str,
    results: &mut Vec<ChatContextSearchResult>,
) {
    if let Some((relative_path, matched_by)) = context_search_match(root, path, query) {
        results.push(ChatContextSearchResult {
            attachment: describe_attachment_path(path.to_string_lossy().to_string()),
            relative_path,
            matched_by,
        });
    }
}

fn search_context_browse_dir(
    root: &Path,
    query: &str,
    limit: usize,
) -> Vec<ChatContextSearchResult> {
    let Some((relative_dir, normalized_query)) = context_browse_dir_from_query(query) else {
        return Vec::new();
    };
    let dir = root.join(relative_dir);
    if !dir.is_dir() {
        return Vec::new();
    }
    let allow_hidden = context_query_allows_hidden(query);
    let mut results = Vec::new();
    let mut scanned = 0usize;
    let Ok(children) = sorted_child_paths(&dir) else {
        return results;
    };
    for path in children {
        if scanned >= CONTEXT_SEARCH_SCAN_LIMIT || results.len() >= limit {
            break;
        }
        scanned += 1;
        if !allow_hidden && is_hidden_context_path(&path) {
            continue;
        }
        push_context_search_result(root, &path, &normalized_query, &mut results);
    }
    results
}

fn search_context_project(root: &Path, query: &str, limit: usize) -> Vec<ChatContextSearchResult> {
    let allow_hidden = context_query_allows_hidden(query);
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(!allow_hidden)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .ignore(true)
        .parents(true);

    let filter_root = root.to_path_buf();
    let mut results = Vec::new();
    let mut scanned = 0usize;
    builder.filter_entry(move |entry| {
        entry.path() == filter_root || !should_skip_context_search_dir(entry.path())
    });
    let walker = builder.build();

    for entry in walker {
        if scanned >= CONTEXT_SEARCH_SCAN_LIMIT || results.len() >= limit {
            break;
        }
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path == root {
            continue;
        }
        scanned += 1;
        push_context_search_result(root, path, query, &mut results);
    }
    results
}

#[tauri::command]
pub fn chat_describe_attachments(paths: Vec<String>) -> Result<Vec<ChatAttachment>, String> {
    Ok(paths
        .into_iter()
        .filter(|path| !path.trim().is_empty())
        .map(describe_attachment_path)
        .collect())
}

#[cfg(windows)]
fn read_windows_clipboard_file_paths() -> Result<Vec<String>, String> {
    use clipboard_win::{formats::FileList, Clipboard, Getter};

    let _clipboard = Clipboard::new_attempts(10).map_err(|e| format!("打开剪贴板失败：{e}"))?;
    let mut paths = Vec::<String>::new();
    match FileList.read_clipboard(&mut paths) {
        Ok(_) => Ok(paths),
        Err(_) => Ok(Vec::new()),
    }
}

#[cfg(not(windows))]
fn read_windows_clipboard_file_paths() -> Result<Vec<String>, String> {
    Ok(Vec::new())
}

#[tauri::command]
pub fn chat_read_clipboard_file_paths() -> Result<Vec<String>, String> {
    read_windows_clipboard_file_paths().map(|paths| {
        paths
            .into_iter()
            .filter(|path| !path.trim().is_empty())
            .collect()
    })
}

#[tauri::command]
pub fn chat_save_clipboard_image(input: ClipboardImageInput) -> Result<ChatAttachment, String> {
    let home = store::resolve_lilia_home();
    let display_seq = CLIPBOARD_IMAGE_DISPLAY_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    save_clipboard_image_to_cache(&home, input, now_millis(), display_seq)
}

#[tauri::command]
pub fn chat_save_clipboard_text(input: ClipboardTextInput) -> Result<ChatAttachment, String> {
    let home = store::resolve_lilia_home();
    let display_seq = CLIPBOARD_TEXT_DISPLAY_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    save_clipboard_text_to_cache(&home, input, now_millis(), display_seq)
}

#[tauri::command]
pub fn chat_search_context_attachments(
    project_cwd: String,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<ChatContextSearchResult>, String> {
    let root = PathBuf::from(project_cwd.trim());
    if root.as_os_str().is_empty() || !root.is_dir() {
        return Ok(Vec::new());
    }
    let limit = limit
        .unwrap_or(DEFAULT_CONTEXT_SEARCH_LIMIT)
        .clamp(1, MAX_CONTEXT_SEARCH_LIMIT);
    let query = query.trim();
    let results = if context_query_is_path_like(query) {
        search_context_browse_dir(&root, query, limit)
    } else {
        search_context_project(&root, query, limit)
    };

    Ok(results)
}

#[cfg(test)]
mod context_search_tests {
    use super::*;

    fn temp_context_root(name: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("lilia-context-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn relative_paths(results: &[ChatContextSearchResult]) -> Vec<String> {
        results
            .iter()
            .map(|result| result.relative_path.clone())
            .collect()
    }

    #[test]
    fn clipboard_image_extension_uses_mime_then_safe_name_extension() {
        assert_eq!(
            clipboard_image_extension(Some("image/jpeg"), Some("ignored.png")),
            "jpg"
        );
        assert_eq!(clipboard_image_extension(None, Some("screen.webp")), "webp");
        assert_eq!(
            clipboard_image_extension(Some("text/plain"), Some("note.txt")),
            "png"
        );
    }

    #[test]
    fn clipboard_image_display_name_uses_short_sequence_name() {
        assert_eq!(clipboard_image_display_name("png", 1), "图片 1.png");
        assert_eq!(clipboard_image_display_name("webp", 12), "图片 12.webp");
    }

    #[test]
    fn clipboard_text_display_name_uses_short_sequence_name() {
        assert_eq!(clipboard_text_display_name(1), "粘贴文本 1.txt");
        assert_eq!(clipboard_text_display_name(12), "粘贴文本 12.txt");
    }

    #[test]
    fn clipboard_image_is_saved_under_cache_and_described_as_image_file() {
        let home = temp_context_root("clipboard-image");
        let input = ClipboardImageInput {
            mime: Some("image/png".to_string()),
            bytes_base64: general_purpose::STANDARD.encode([1_u8, 2, 3, 4]),
            name: None,
        };

        let attachment = save_clipboard_image_to_cache(&home, input, 12345, 1).unwrap();

        assert_eq!(attachment.name, "图片 1.png");
        assert_eq!(attachment.kind, "file");
        assert_eq!(attachment.exists, true);
        assert_eq!(attachment.mime.as_deref(), Some("image/png"));
        let path = PathBuf::from(&attachment.path);
        assert!(path.exists());
        assert_eq!(
            path.parent().unwrap(),
            clipboard_images_cache_dir(&home).as_path()
        );
        assert_eq!(fs::read(&path).unwrap(), vec![1_u8, 2, 3, 4]);

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn clipboard_text_is_saved_under_cache_and_described_as_file() {
        let home = temp_context_root("clipboard-text");
        let input = ClipboardTextInput {
            text: "很长的粘贴文本\nwith ascii".to_string(),
        };

        let attachment = save_clipboard_text_to_cache(&home, input, 12345, 1).unwrap();

        assert_eq!(attachment.name, "粘贴文本 1.txt");
        assert_eq!(attachment.kind, "file");
        assert_eq!(attachment.exists, true);
        assert_eq!(attachment.mime, None);
        assert_eq!(
            attachment.size,
            Some("很长的粘贴文本\nwith ascii".len() as u64)
        );
        let path = PathBuf::from(&attachment.path);
        assert!(path.exists());
        assert_eq!(
            path.parent().unwrap(),
            clipboard_texts_cache_dir(&home).as_path()
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "很长的粘贴文本\nwith ascii"
        );

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn context_search_hides_dot_files_until_query_contains_dot() {
        let root = temp_context_root("hidden");
        write_file(&root.join(".env"), "token=1");
        write_file(&root.join("src").join("env.ts"), "export {}");

        let hidden = relative_paths(
            &chat_search_context_attachments(
                root.to_string_lossy().to_string(),
                "env".to_string(),
                Some(20),
            )
            .unwrap(),
        );
        assert!(!hidden.contains(&".env".to_string()));
        assert!(hidden.contains(&"src/env.ts".to_string()));

        let explicit = relative_paths(
            &chat_search_context_attachments(
                root.to_string_lossy().to_string(),
                ".".to_string(),
                Some(20),
            )
            .unwrap(),
        );
        assert!(explicit.contains(&".env".to_string()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn context_search_hides_gitignored_entries_until_path_query() {
        let root = temp_context_root("ignored");
        write_file(&root.join(".gitignore"), "dist/\n");
        write_file(&root.join("dist").join("app.js"), "console.log(1)");
        write_file(&root.join("src").join("dist-note.md"), "note");

        let hidden = relative_paths(
            &chat_search_context_attachments(
                root.to_string_lossy().to_string(),
                "dist".to_string(),
                Some(20),
            )
            .unwrap(),
        );
        assert!(!hidden.contains(&"dist".to_string()));
        assert!(!hidden.contains(&"dist/app.js".to_string()));
        assert!(hidden.contains(&"src/dist-note.md".to_string()));

        let explicit = relative_paths(
            &chat_search_context_attachments(
                root.to_string_lossy().to_string(),
                "dist/".to_string(),
                Some(20),
            )
            .unwrap(),
        );
        assert!(explicit.contains(&"dist/app.js".to_string()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn context_path_query_lists_direct_children_only() {
        let root = temp_context_root("browse");
        write_file(&root.join("big-dir").join("inside.md"), "inside");
        write_file(&root.join("big-dir").join("nested").join("deep.md"), "deep");

        let paths = relative_paths(
            &chat_search_context_attachments(
                root.to_string_lossy().to_string(),
                "big-dir/".to_string(),
                Some(20),
            )
            .unwrap(),
        );
        assert!(paths.contains(&"big-dir/inside.md".to_string()));
        assert!(paths.contains(&"big-dir/nested".to_string()));
        assert!(!paths.contains(&"big-dir/nested/deep.md".to_string()));

        let _ = fs::remove_dir_all(root);
    }
}
