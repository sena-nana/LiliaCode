//! How one timeline event reads: its kind of step, icon, label and tone.

use nana_ui::icons_tabler as tabler;
use nana_ui::runtime::SemanticColorRole;
use nana_ui::Icon;

/// What an entry is in the conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineRole {
    /// The person's message, drawn as a bubble on the right.
    User,
    /// The agent's answer, drawn as plain prose on the rail.
    Reply,
    /// One process step (tool call, command, plan, …).
    Step(StepKind),
    /// Consecutive process steps folded into one entry.
    Group,
}

/// Where a step stands; drives the rail icon colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TimelineTone {
    #[default]
    Settled,
    Running,
    Waiting,
    Failed,
}

impl TimelineTone {
    pub(crate) fn from_status(status: &str) -> Self {
        match status {
            "running" | "started" | "in_progress" | "streaming" | "starting" => Self::Running,
            "pending" | "requires_action" | "waiting" | "queued" => Self::Waiting,
            "failed" | "error" | "cancelled" | "interrupted" => Self::Failed,
            _ => Self::Settled,
        }
    }

    pub(crate) fn icon_role(self) -> SemanticColorRole {
        match self {
            Self::Settled => SemanticColorRole::Faint,
            Self::Running => SemanticColorRole::Accent,
            Self::Waiting => SemanticColorRole::Warning,
            Self::Failed => SemanticColorRole::Danger,
        }
    }

    pub(crate) fn title_role(self) -> SemanticColorRole {
        match self {
            Self::Settled => SemanticColorRole::Muted,
            Self::Running => SemanticColorRole::Text,
            Self::Waiting => SemanticColorRole::Warning,
            Self::Failed => SemanticColorRole::Danger,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepKind {
    Thinking,
    Run,
    Read,
    Edit,
    Search,
    Web,
    Agent,
    Plan,
    Todo,
    Ask,
    Architecture,
    Mcp,
    Hook,
    Tool,
    FileChange,
    Usage,
    Status,
    Goal,
    Title,
    Diagnostics,
    Error,
    Note,
}

impl StepKind {
    /// Classifies an event by its projection kind and, for tool calls, the
    /// tool's name.
    pub(crate) fn classify(kind: &str, tool: Option<&str>, status: &str) -> Self {
        let step = match kind {
            "reasoning" => Self::Thinking,
            "command" => Self::Run,
            "subagent" => Self::Agent,
            "plan" => Self::Plan,
            "todo_list" | "todo" => Self::Todo,
            "file_change" => Self::FileChange,
            "usage" => Self::Usage,
            "turn_state" => Self::Status,
            "goal" => Self::Goal,
            "title" | "title_update" => Self::Title,
            "diagnostics" => Self::Diagnostics,
            "architecture" => Self::Architecture,
            "hook" => Self::Hook,
            "question" | "ask_user" | "interaction" => Self::Ask,
            "tool" | "tool_call" | "tool_result" => tool.map(Self::from_tool).unwrap_or(Self::Tool),
            "error" => Self::Error,
            _ => Self::Note,
        };
        if TimelineTone::from_status(status) == TimelineTone::Failed && step == Self::Note {
            Self::Error
        } else {
            step
        }
    }

    fn from_tool(name: &str) -> Self {
        let name = name.to_ascii_lowercase();
        let words = name
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        let has = |needles: &[&str]| {
            words
                .iter()
                .any(|word| needles.iter().any(|needle| word.starts_with(needle)))
        };
        if name.starts_with("mcp__") || name.starts_with("mcp.") {
            Self::Mcp
        } else if has(&["delegate", "subagent", "agent"]) {
            Self::Agent
        } else if has(&["todo"]) {
            Self::Todo
        } else if has(&["plan"]) {
            Self::Plan
        } else if has(&["ask", "question", "elicit"]) {
            Self::Ask
        } else if has(&["architecture"]) {
            Self::Architecture
        } else if has(&["hook"]) {
            Self::Hook
        } else if has(&["fetch", "web", "browser", "url", "http"]) {
            Self::Web
        } else if has(&["edit", "write", "patch", "replace", "create", "apply"]) {
            Self::Edit
        } else if has(&["grep", "glob", "search", "find", "list", "ls"]) {
            Self::Search
        } else if has(&["read", "view", "open", "cat"]) {
            Self::Read
        } else if has(&["bash", "shell", "exec", "command", "run", "terminal"]) {
            Self::Run
        } else {
            Self::Tool
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Thinking => "思考",
            Self::Run => "运行",
            Self::Read => "读取",
            Self::Edit => "修改",
            Self::Search => "搜索",
            Self::Web => "抓取网页",
            Self::Agent => "调用子代理",
            Self::Plan => "制定计划",
            Self::Todo => "更新待办",
            Self::Ask => "提问",
            Self::Architecture => "更新架构",
            Self::Mcp => "MCP",
            Self::Hook => "运行 Hook",
            Self::Tool => "调用工具",
            Self::FileChange => "修改文件",
            Self::Usage => "用量",
            Self::Status => "执行状态",
            Self::Goal => "目标",
            Self::Title => "标题已更新",
            Self::Diagnostics => "诊断",
            Self::Error => "发生错误",
            Self::Note => "事件",
        }
    }

    pub(crate) fn icon(self) -> Icon {
        match self {
            Self::Thinking => tabler::BULB,
            Self::Run => tabler::TERMINAL_2,
            Self::Read => tabler::BOOK,
            Self::Edit => tabler::FILE_PENCIL,
            Self::Search => tabler::SEARCH,
            Self::Web => tabler::WORLD,
            Self::Agent => tabler::ROBOT,
            Self::Plan => tabler::LIST_NUMBERS,
            Self::Todo => tabler::LIST_CHECK,
            Self::Ask => tabler::HELP_CIRCLE,
            Self::Architecture => tabler::SITEMAP,
            Self::Mcp => tabler::PLUG,
            Self::Hook => tabler::WEBHOOK,
            Self::Tool => tabler::TOOL,
            Self::FileChange => tabler::FILE_DIFF,
            Self::Usage => tabler::GAUGE,
            Self::Status => tabler::CIRCLE_DOT,
            Self::Goal => tabler::TARGET,
            Self::Title => tabler::CURSOR_TEXT,
            Self::Diagnostics => tabler::STETHOSCOPE,
            Self::Error => tabler::ALERT_TRIANGLE,
            Self::Note => tabler::POINT,
        }
    }

    /// Steps that are the agent working rather than talking: they fold into
    /// a process group and read in the muted title colour.
    pub(crate) fn is_process(self) -> bool {
        !matches!(self, Self::Error | Self::Ask)
    }
}

impl TimelineRole {
    pub(crate) fn icon(self) -> Icon {
        match self {
            Self::User => tabler::MESSAGE,
            Self::Reply => tabler::SPARKLES,
            Self::Step(kind) => kind.icon(),
            Self::Group => tabler::LIST_DETAILS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names_read_as_the_operation_they_perform() {
        let classify = |tool| StepKind::classify("tool", Some(tool), "success");
        assert_eq!(classify("read_file"), StepKind::Read);
        assert_eq!(classify("str_replace_edit"), StepKind::Edit);
        assert_eq!(classify("grep"), StepKind::Search);
        assert_eq!(classify("bash"), StepKind::Run);
        assert_eq!(classify("web_fetch"), StepKind::Web);
        assert_eq!(classify("delegate_agent"), StepKind::Agent);
        assert_eq!(classify("mcp__github__issues"), StepKind::Mcp);
        assert_eq!(classify("something_else"), StepKind::Tool);
        assert_eq!(StepKind::classify("tool", None, "success"), StepKind::Tool);
    }

    #[test]
    fn failed_unknown_events_surface_as_errors() {
        assert_eq!(StepKind::classify("note", None, "error"), StepKind::Error);
        assert_eq!(StepKind::classify("command", None, "error"), StepKind::Run);
        assert_eq!(
            TimelineTone::from_status("error").icon_role(),
            SemanticColorRole::Danger
        );
        assert_eq!(
            TimelineTone::from_status("running").icon_role(),
            SemanticColorRole::Accent
        );
    }
}
