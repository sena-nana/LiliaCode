//! Line based document diffs used by the native editor review surface.
//!
//! The document buffer remains the authority for edits.  A diff is an
//! immutable projection of two revisions, so accepting or rejecting a hunk
//! cannot accidentally mutate a live buffer.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: DiffLineKind,
    /// One based line number in the old revision, when the line exists there.
    pub old_line: Option<usize>,
    /// One based line number in the new revision, when the line exists there.
    pub new_line: Option<usize>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_len: usize,
    pub new_start: usize,
    pub new_len: usize,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDiff {
    pub hunks: Vec<DiffHunk>,
}

impl DocumentDiff {
    pub fn between(old: &str, new: &str) -> Self {
        Self {
            hunks: diff_hunks(old, new, 3),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.hunks.is_empty()
    }

    /// Splits a unified patch (`git diff` output) into one diff per file, in
    /// patch order. The path is the new side's, or the old side's for a
    /// deletion. Malformed hunk headers end that file's hunks rather than
    /// guessing line numbers.
    pub fn parse_unified(patch: &str) -> Vec<(String, DocumentDiff)> {
        let mut files: Vec<(String, DocumentDiff)> = Vec::new();
        let mut old_path: Option<String> = None;
        let mut lines: Option<(usize, usize)> = None;
        for line in patch.lines() {
            if line.starts_with("diff --git ") {
                old_path = None;
                lines = None;
                continue;
            }
            if let Some(path) = line.strip_prefix("--- ") {
                old_path = patch_path(path, "a/");
                lines = None;
                continue;
            }
            if let Some(path) = line.strip_prefix("+++ ") {
                let path = patch_path(path, "b/").or_else(|| old_path.clone());
                if let Some(path) = path {
                    files.push((path, DocumentDiff { hunks: Vec::new() }));
                }
                lines = None;
                continue;
            }
            if line.starts_with("@@") {
                lines = None;
                let Some((_, diff)) = files.last_mut() else {
                    continue;
                };
                let Some(header) = parse_hunk_header(line) else {
                    continue;
                };
                lines = Some((header.old_start, header.new_start));
                diff.hunks.push(header);
                continue;
            }
            let (Some((old_line, new_line)), Some((_, diff))) = (lines.as_mut(), files.last_mut())
            else {
                continue;
            };
            let Some(hunk) = diff.hunks.last_mut() else {
                continue;
            };
            let (kind, text) = match line.as_bytes().first() {
                Some(b'+') => (DiffLineKind::Added, &line[1..]),
                Some(b'-') => (DiffLineKind::Removed, &line[1..]),
                Some(b' ') => (DiffLineKind::Context, &line[1..]),
                Some(b'\\') => continue,
                None => (DiffLineKind::Context, ""),
                _ => continue,
            };
            let (old, new) = match kind {
                DiffLineKind::Added => (None, Some(*new_line)),
                DiffLineKind::Removed => (Some(*old_line), None),
                DiffLineKind::Context => (Some(*old_line), Some(*new_line)),
            };
            if old.is_some() {
                *old_line += 1;
            }
            if new.is_some() {
                *new_line += 1;
            }
            hunk.lines.push(DiffLine {
                kind,
                old_line: old,
                new_line: new,
                text: text.to_owned(),
            });
        }
        files
    }

    /// Lines this diff adds and removes.
    pub fn line_counts(&self) -> (usize, usize) {
        self.hunks
            .iter()
            .flat_map(|hunk| &hunk.lines)
            .fold((0, 0), |(added, removed), line| match line.kind {
                DiffLineKind::Added => (added + 1, removed),
                DiffLineKind::Removed => (added, removed + 1),
                DiffLineKind::Context => (added, removed),
            })
    }
}

fn patch_path(raw: &str, prefix: &str) -> Option<String> {
    let raw = raw.split('\t').next().unwrap_or(raw).trim();
    if raw == "/dev/null" {
        return None;
    }
    Some(raw.strip_prefix(prefix).unwrap_or(raw).to_owned())
}

fn parse_hunk_header(line: &str) -> Option<DiffHunk> {
    let ranges = line.strip_prefix("@@ ")?.split(" @@").next()?;
    let mut parts = ranges.split_whitespace();
    let (old_start, old_len) = parse_range(parts.next()?.strip_prefix('-')?)?;
    let (new_start, new_len) = parse_range(parts.next()?.strip_prefix('+')?)?;
    Some(DiffHunk {
        old_start,
        old_len,
        new_start,
        new_len,
        lines: Vec::new(),
    })
}

fn parse_range(range: &str) -> Option<(usize, usize)> {
    match range.split_once(',') {
        Some((start, len)) => Some((start.parse().ok()?, len.parse().ok()?)),
        None => Some((range.parse().ok()?, 1)),
    }
}

#[derive(Clone, Copy, Debug)]
enum OpKind {
    Context,
    Added,
    Removed,
}

#[derive(Clone, Debug)]
struct Op {
    kind: OpKind,
    old: Option<usize>,
    new: Option<usize>,
    text: String,
}

fn lines(text: &str) -> Vec<&str> {
    // split() retains a final empty line, which keeps a trailing newline
    // observable in the review rather than silently dropping it.
    if text.is_empty() {
        Vec::new()
    } else {
        text.split('\n').collect()
    }
}

fn diff_hunks(old: &str, new: &str, context: usize) -> Vec<DiffHunk> {
    let old_lines = lines(old);
    let new_lines = lines(new);
    let n = old_lines.len();
    let m = new_lines.len();

    // A full matrix is predictable and ideal for editor-sized files.  Avoid
    // an unbounded allocation for generated/binary-like inputs; those are
    // represented as one replacement hunk instead.
    if n.saturating_mul(m) > 4_000_000 {
        return replacement_hunk(&old_lines, &new_lines);
    }

    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if old_lines[i] == new_lines[j] {
                lcs[i + 1][j + 1].saturating_add(1)
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut ops = Vec::with_capacity(n.saturating_add(m));
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if old_lines[i] == new_lines[j] {
            ops.push(Op {
                kind: OpKind::Context,
                old: Some(i),
                new: Some(j),
                text: old_lines[i].to_owned(),
            });
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            ops.push(Op {
                kind: OpKind::Removed,
                old: Some(i),
                new: None,
                text: old_lines[i].to_owned(),
            });
            i += 1;
        } else {
            ops.push(Op {
                kind: OpKind::Added,
                old: None,
                new: Some(j),
                text: new_lines[j].to_owned(),
            });
            j += 1;
        }
    }
    while i < n {
        ops.push(Op {
            kind: OpKind::Removed,
            old: Some(i),
            new: None,
            text: old_lines[i].to_owned(),
        });
        i += 1;
    }
    while j < m {
        ops.push(Op {
            kind: OpKind::Added,
            old: None,
            new: Some(j),
            text: new_lines[j].to_owned(),
        });
        j += 1;
    }

    let changed = ops
        .iter()
        .enumerate()
        .filter_map(|(index, op)| (!matches!(op.kind, OpKind::Context)).then_some(index))
        .collect::<Vec<_>>();
    if changed.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::<(usize, usize)>::new();
    let mut start = changed[0].saturating_sub(context);
    let mut end = (changed[0] + context + 1).min(ops.len());
    for &index in &changed[1..] {
        let candidate_start = index.saturating_sub(context);
        let candidate_end = (index + context + 1).min(ops.len());
        if candidate_start <= end {
            end = end.max(candidate_end);
        } else {
            ranges.push((start, end));
            start = candidate_start;
            end = candidate_end;
        }
    }
    ranges.push((start, end));

    ranges
        .into_iter()
        .map(|(start, end)| {
            let slice = &ops[start..end];
            let old_start = slice
                .iter()
                .find_map(|op| op.old)
                .map_or(n + 1, |line| line + 1);
            let new_start = slice
                .iter()
                .find_map(|op| op.new)
                .map_or(m + 1, |line| line + 1);
            let old_len = slice.iter().filter(|op| op.old.is_some()).count();
            let new_len = slice.iter().filter(|op| op.new.is_some()).count();
            let lines = slice
                .iter()
                .map(|op| DiffLine {
                    kind: match op.kind {
                        OpKind::Context => DiffLineKind::Context,
                        OpKind::Added => DiffLineKind::Added,
                        OpKind::Removed => DiffLineKind::Removed,
                    },
                    old_line: op.old.map(|line| line + 1),
                    new_line: op.new.map(|line| line + 1),
                    text: op.text.clone(),
                })
                .collect();
            DiffHunk {
                old_start,
                old_len,
                new_start,
                new_len,
                lines,
            }
        })
        .collect()
}

fn replacement_hunk(old: &[&str], new: &[&str]) -> Vec<DiffHunk> {
    if old == new {
        return Vec::new();
    }
    let mut lines = old
        .iter()
        .enumerate()
        .map(|(index, text)| DiffLine {
            kind: DiffLineKind::Removed,
            old_line: Some(index + 1),
            new_line: None,
            text: (*text).to_owned(),
        })
        .collect::<Vec<_>>();
    lines.extend(new.iter().enumerate().map(|(index, text)| DiffLine {
        kind: DiffLineKind::Added,
        old_line: None,
        new_line: Some(index + 1),
        text: (*text).to_owned(),
    }));
    vec![DiffHunk {
        old_start: 1,
        old_len: old.len(),
        new_start: 1,
        new_len: new.len(),
        lines,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_patch() -> String {
        [
            "diff --git a/src/lib.rs b/src/lib.rs",
            "index 1111111..2222222 100644",
            "--- a/src/lib.rs",
            "+++ b/src/lib.rs",
            "@@ -1,3 +1,4 @@",
            " fn main() {",
            "-    old();",
            "+    new();",
            "+    more();",
            " }",
            "diff --git a/gone.txt b/gone.txt",
            "deleted file mode 100644",
            "--- a/gone.txt",
            "+++ /dev/null",
            "@@ -1 +0,0 @@",
            "-bye",
        ]
        .join("\n")
    }

    #[test]
    fn unified_patches_split_into_per_file_diffs_with_line_numbers() {
        let files = DocumentDiff::parse_unified(&sample_patch());
        assert_eq!(
            files
                .iter()
                .map(|(path, _)| path.as_str())
                .collect::<Vec<_>>(),
            ["src/lib.rs", "gone.txt"]
        );
        let (_, lib) = &files[0];
        assert_eq!(lib.line_counts(), (2, 1));
        let hunk = &lib.hunks[0];
        assert_eq!((hunk.old_start, hunk.new_start), (1, 1));
        assert_eq!(
            hunk.lines
                .iter()
                .map(|line| (line.kind, line.old_line, line.new_line))
                .collect::<Vec<_>>(),
            [
                (DiffLineKind::Context, Some(1), Some(1)),
                (DiffLineKind::Removed, Some(2), None),
                (DiffLineKind::Added, None, Some(2)),
                (DiffLineKind::Added, None, Some(3)),
                (DiffLineKind::Context, Some(3), Some(4)),
            ]
        );
        assert_eq!(files[1].1.line_counts(), (0, 1));
    }

    #[test]
    fn malformed_hunk_headers_drop_their_lines() {
        let files = DocumentDiff::parse_unified(
            &["--- a/x", "+++ b/x", "@@ broken @@", "+ignored"].join("\n"),
        );
        assert_eq!(files.len(), 1);
        assert!(files[0].1.is_empty());
    }

    #[test]
    fn reports_added_removed_lines_with_context() {
        let diff = DocumentDiff::between("one\ntwo\nthree", "one\nchanged\nthree\nfour");
        assert_eq!(diff.hunks.len(), 1);
        let lines = &diff.hunks[0].lines;
        assert!(lines
            .iter()
            .any(|line| { line.kind == DiffLineKind::Removed && line.text == "two" }));
        assert!(lines
            .iter()
            .any(|line| { line.kind == DiffLineKind::Added && line.text == "changed" }));
        assert_eq!(diff.hunks[0].new_len, 4);
    }

    #[test]
    fn equal_documents_have_no_review_hunks() {
        assert!(DocumentDiff::between("same", "same").is_empty());
    }

    #[test]
    fn trailing_newline_is_part_of_the_diff() {
        let diff = DocumentDiff::between("line", "line\n");
        assert!(!diff.is_empty());
        assert_eq!(
            diff.hunks[0].lines.last().unwrap().kind,
            DiffLineKind::Added
        );
    }
}
