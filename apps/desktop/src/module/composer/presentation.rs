#[derive(Debug, Clone, PartialEq)]
pub struct ComposerAttachment {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComposerSuggestion {
    pub id: String,
    pub label: String,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComposerSlashItem {
    pub name: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComposerMentionItem {
    pub id: String,
    pub label: String,
}

pub(crate) const COMPOSER_PERMISSION_OPTIONS: [(&str, &str); 3] =
    [("ask", "询问"), ("readonly", "只读"), ("full", "完全")];

pub(crate) const COMPOSER_WORKTREE_OPTIONS: [(&str, &str); 3] = [
    ("current", "当前仓库"),
    ("create", "新建工作树"),
    ("existing", "已有工作树…"),
];

pub(crate) const COMPOSER_REASONING_OPTIONS: [(&str, &str); 5] = [
    ("low", "低"),
    ("medium", "中"),
    ("high", "高"),
    ("xhigh", "超高"),
    ("max", "最高"),
];

pub(crate) const COMPOSER_REVIEW_OPTIONS: [(&str, &str); 3] = [
    ("changes", "未提交的改动"),
    ("branch", "与分支比较"),
    ("commit", "指定提交"),
];

pub(crate) fn reasoning_selection(effort: Option<&str>) -> &'static str {
    match effort {
        Some("low") => "low",
        Some("high") => "high",
        Some("xhigh") => "xhigh",
        Some("max") => "max",
        _ => "medium",
    }
}

pub(crate) fn permission_selection(
    permission: crate::application::DesktopExecutionPermission,
) -> (&'static str, &'static str) {
    use crate::application::DesktopExecutionPermission;
    match permission {
        DesktopExecutionPermission::Ask => COMPOSER_PERMISSION_OPTIONS[0],
        DesktopExecutionPermission::Readonly => COMPOSER_PERMISSION_OPTIONS[1],
        DesktopExecutionPermission::Full => COMPOSER_PERMISSION_OPTIONS[2],
    }
}

pub(crate) fn permission_from_selection_id(
    id: &str,
) -> Option<crate::application::DesktopExecutionPermission> {
    use crate::application::DesktopExecutionPermission;
    match COMPOSER_PERMISSION_OPTIONS
        .iter()
        .position(|(key, _)| *key == id)
    {
        Some(0) => Some(DesktopExecutionPermission::Ask),
        Some(1) => Some(DesktopExecutionPermission::Readonly),
        Some(2) => Some(DesktopExecutionPermission::Full),
        _ => None,
    }
}
