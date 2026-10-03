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
