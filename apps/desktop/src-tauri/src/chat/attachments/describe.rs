use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::chat::types::{ChatAttachment, ChatAttachmentDirectoryMeta};

const DIRECTORY_SCAN_LIMIT: usize = 1000;

pub(super) fn image_mime_for_path(path: &Path) -> Option<String> {
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

pub(super) fn describe_attachment_path(path: String) -> ChatAttachment {
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
