use std::fs;
use std::path::{Component, Path, PathBuf};

use ignore::WalkBuilder;

use crate::chat::attachments::describe::describe_attachment_path;
use crate::chat::types::ChatContextSearchResult;

const CONTEXT_SEARCH_SCAN_LIMIT: usize = 6000;
const DEFAULT_CONTEXT_SEARCH_LIMIT: usize = 12;
const MAX_CONTEXT_SEARCH_LIMIT: usize = 50;

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

pub(super) fn context_query_is_path_like(query: &str) -> bool {
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

pub(super) fn search_context_attachments(
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
mod tests {
    use super::*;
    use std::env;
    use uuid::Uuid;

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
    fn context_search_hides_dot_files_until_query_contains_dot() {
        let root = temp_context_root("hidden");
        write_file(&root.join(".env"), "token=1");
        write_file(&root.join("src").join("env.ts"), "export {}");

        let hidden = relative_paths(
            &search_context_attachments(
                root.to_string_lossy().to_string(),
                "env".to_string(),
                Some(20),
            )
            .unwrap(),
        );
        assert!(!hidden.contains(&".env".to_string()));
        assert!(hidden.contains(&"src/env.ts".to_string()));

        let explicit = relative_paths(
            &search_context_attachments(
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
            &search_context_attachments(
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
            &search_context_attachments(
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
            &search_context_attachments(
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
