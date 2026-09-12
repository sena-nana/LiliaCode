//! Timeline view state as a UI module.
//!
//! The events themselves live on the task session the shell publishes. This
//! module owns expansion state for its window.

use std::collections::BTreeSet;

use lilia_kernel::FeatureId;

use crate::runtime_shell::{PrimaryShellSnapshot, ShellTimelineRow};
use crate::task_session::TaskTimelineItem;
use crate::ui_module::{UiModule, UiModuleContext, UiModuleOutcome};

#[derive(Debug, Clone)]
pub struct TimelineTextSelection {
    pub event_id: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum TimelineModuleMessage {
    Toggle(String),
    SelectText {
        event_id: String,
        text: Option<String>,
    },
    ClearTextSelection,
}

pub struct TimelineModule {
    toggled_events: BTreeSet<String>,
    text_selection: Option<TimelineTextSelection>,
}

impl Default for TimelineModule {
    fn default() -> Self {
        Self {
            toggled_events: BTreeSet::new(),
            text_selection: None,
        }
    }
}

impl TimelineModule {
    pub fn feature_id() -> FeatureId {
        FeatureId::new("lilia.timeline").expect("the timeline feature id is not blank")
    }

    pub fn text_selection(&self) -> Option<&TimelineTextSelection> {
        self.text_selection.as_ref()
    }

    pub fn is_toggled(&self, id: &str) -> bool {
        self.toggled_events.contains(id)
    }

    pub fn rows(
        session: &crate::task_session::TaskSessionView,
        toggled: impl Fn(&str) -> bool,
    ) -> Vec<ShellTimelineRow> {
        let mut rows = Vec::new();
        let mut index = 0;
        while index < session.timeline.len() {
            let item = &session.timeline[index];
            if item.kind == "turn_state"
                && item.turn_id.is_some()
                && session.timeline[index + 1..]
                    .iter()
                    .any(|later| later.kind == "turn_state" && later.turn_id == item.turn_id)
            {
                index += 1;
                continue;
            }
            let mut end = index + 1;
            if is_process_event(item) {
                while end < session.timeline.len()
                    && is_process_event(&session.timeline[end])
                    && session.timeline[end].turn_id == item.turn_id
                {
                    end += 1;
                }
            }
            if end > index + 1 {
                let group_id = format!("process-group:{}", item.id);
                let expanded = toggled(&group_id);
                rows.push(ShellTimelineRow {
                    selected_text: None,
                    attachments: Vec::new(),
                    images: Vec::new(),
                    id: group_id,
                    title: format!("执行过程 · {} 项", end - index),
                    role: "process_group".into(),
                    status: process_status(&session.timeline[index..end]).into(),
                    markdown: String::new(),
                    expanded,
                    can_expand: true,
                    can_copy: false,
                    can_retry: false,
                    can_branch: false,
                    can_apply: false,
                });
                if expanded {
                    for child in &session.timeline[index..end] {
                        let mut row = Self::row(child, toggled(&child.id), false);
                        row.role = format!("process-child:{}", row.role);
                        rows.push(row);
                    }
                }
            } else {
                let mut row = Self::row(
                    item,
                    toggled(&item.id),
                    item.can_retry && session.run_block.is_none(),
                );
                if session.blocking_pending_count > 0 || session.run_block.is_some() {
                    row.can_branch = false;
                    row.can_apply = false;
                }
                rows.push(row);
            }
            index = end;
        }
        rows
    }

    pub fn row(item: &TaskTimelineItem, expanded: bool, can_retry: bool) -> ShellTimelineRow {
        let expanded =
            matches!(item.message_role.as_deref(), Some("user" | "assistant")) ^ expanded;
        let full = item
            .markdown
            .clone()
            .or_else(|| item.summary.clone())
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| item.title.clone());
        let preview = item
            .summary
            .clone()
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| item.title.clone());
        let can_expand = item.message_role.is_none()
            && item.markdown.as_ref().is_some_and(|markdown| {
                item.summary
                    .as_ref()
                    .is_some_and(|summary| summary != markdown)
                    || item.title != *markdown
            });
        ShellTimelineRow {
            selected_text: None,
            attachments: item.attachments.clone(),
            images: Vec::new(),
            id: item.id.clone(),
            title: item.title.clone(),
            role: item
                .message_role
                .clone()
                .unwrap_or_else(|| item.kind.clone()),
            status: item.status.clone(),
            can_branch: item.session_branch_turn_id.is_some(),
            can_apply: item.batch_apply.is_some(),
            markdown: if expanded || !can_expand {
                full
            } else {
                preview
            },
            expanded,
            can_expand,
            can_retry,
            can_copy: item
                .markdown_plain_text
                .as_ref()
                .or(item.markdown.as_ref())
                .is_some_and(|text| !text.trim().is_empty()),
        }
    }
}

impl UiModule for TimelineModule {
    type Message = TimelineModuleMessage;

    fn feature(&self) -> FeatureId {
        Self::feature_id()
    }

    fn reduce(&mut self, message: Self::Message, _cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        match message {
            TimelineModuleMessage::Toggle(event_id) => {
                if !self.toggled_events.remove(&event_id) {
                    self.toggled_events.insert(event_id);
                }
                UiModuleOutcome::dirty()
            }
            TimelineModuleMessage::ClearTextSelection => {
                self.text_selection = None;
                UiModuleOutcome::dirty()
            }
            TimelineModuleMessage::SelectText { event_id, text } => {
                if let Some(text) = text.filter(|text| !text.trim().is_empty()) {
                    self.text_selection = Some(TimelineTextSelection { event_id, text });
                } else if self
                    .text_selection
                    .as_ref()
                    .is_some_and(|selection| selection.event_id == event_id)
                {
                    self.text_selection = None;
                }
                UiModuleOutcome::dirty()
            }
        }
    }

    fn project(&self, cx: &UiModuleContext<'_>, into: &mut PrimaryShellSnapshot) {
        if !crate::module::conversation_is_visible(cx) {
            return;
        }
        let Some(session) = cx.task_session() else {
            into.timeline.clear();
            into.timeline_layout = nana_ui::VirtualListLayout::default();
            into.timeline_can_load_earlier = false;
            return;
        };
        into.timeline_can_load_earlier = session.timeline_has_more_before;
        into.timeline = Self::rows(&session, |id| self.is_toggled(id));
        for row in &mut into.timeline {
            row.selected_text = self
                .text_selection
                .as_ref()
                .filter(|selection| selection.event_id == row.id)
                .map(|selection| selection.text.clone());
        }
        into.timeline_layout = nana_ui::VirtualListLayout::new(into.timeline.iter().map(|row| {
            if row.role == "process_group" {
                return 36.0;
            }
            let width = if row.role == "user" { 54 } else { 72 };
            let lines = row
                .markdown
                .lines()
                .map(|line| line.chars().count().max(1).div_ceil(width))
                .sum::<usize>()
                .max(1);
            lines as f32 * 22.0
                + row.attachments.len() as f32 * 32.0
                + if row.role == "user" { 24.0 } else { 12.0 }
                + if row.role != "user" && row.role != "assistant" {
                    26.0
                } else {
                    0.0
                }
                + if row.can_expand
                    || row.can_copy
                    || row.can_retry
                    || row.can_branch
                    || row.can_apply
                {
                    30.0
                } else {
                    0.0
                }
        }));
    }

    fn invalidate(
        &mut self,
        envelope: &lilia_kernel::EventEnvelope,
        cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        let selected = cx.selected_task();
        let for_selected = |task_id: &lilia_contracts::TaskId| selected.as_ref() == Some(task_id);
        if envelope
            .downcast::<crate::application::TimelineChanged>()
            .is_some_and(|event| for_selected(&event.task_id))
            || envelope
                .downcast::<crate::application::ApprovalChanged>()
                .is_some_and(|event| for_selected(&event.task_id))
            || envelope
                .downcast::<crate::application::InteractionChanged>()
                .is_some_and(|event| for_selected(&event.task_id))
        {
            return UiModuleOutcome::dirty();
        }
        UiModuleOutcome::clean()
    }
}

fn is_process_event(item: &TaskTimelineItem) -> bool {
    item.message_role.is_none()
        && matches!(
            item.kind.as_str(),
            "reasoning" | "tool" | "tool_call" | "tool_result" | "progress"
        )
        && !matches!(item.status.as_str(), "failed" | "error")
        && !item.can_retry
        && item.batch_apply.is_none()
        && item.session_branch_turn_id.is_none()
}

fn process_status(items: &[TaskTimelineItem]) -> &str {
    for statuses in [
        &["failed", "error", "cancelled"][..],
        &[
            "running",
            "started",
            "in_progress",
            "in-progress",
            "pending",
            "queued",
        ][..],
    ] {
        if let Some(item) = items
            .iter()
            .find(|item| statuses.contains(&item.status.as_str()))
        {
            return &item.status;
        }
    }
    items
        .last()
        .map(|item| item.status.as_str())
        .unwrap_or("completed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_session::TaskSessionView;

    fn event(id: &str, kind: &str, status: &str) -> TaskTimelineItem {
        TaskTimelineItem {
            id: id.into(),
            sequence: 1,
            turn_id: Some("turn".into()),
            kind: kind.into(),
            title: format!("操作{id}"),
            message_role: None,
            summary: Some("摘要".into()),
            markdown: Some("完整详情".into()),
            markdown_document: None,
            markdown_plain_text: Some("完整详情".into()),
            markdown_table_count: 0,
            attachments: Vec::new(),
            status: status.into(),
            batch_apply: None,
            session_branch_turn_id: None,
            selectable_reply: false,
            can_retry: false,
        }
    }
    fn session(timeline: Vec<TaskTimelineItem>) -> TaskSessionView {
        TaskSessionView {
            task_title: String::new(),
            run_block: None,
            goal: None,
            context_usage: None,
            timeline,
            timeline_layout: Default::default(),
            timeline_before_cursor: None,
            timeline_has_more_before: true,
            artifact_count: 0,
            todo_count: 0,
            artifacts: Vec::new(),
            todos: Vec::new(),
            worktree: None,
            pending_count: 0,
            open_pending_count: 0,
            blocking_pending_count: 0,
            pending: Vec::new(),
        }
    }
    #[test]
    fn process_groups_aggregate_running_state_and_keep_expanded_event_identity() {
        let mut reply = event("reply", "message", "completed");
        reply.message_role = Some("assistant".into());
        reply.session_branch_turn_id = Some("turn".into());
        let session = session(vec![
            event("read", "tool", "success"),
            event("search", "tool", "running"),
            reply,
        ]);
        let collapsed = TimelineModule::rows(&session, |_| false);
        assert_eq!(collapsed.len(), 2);
        assert_eq!(collapsed[0].id, "process-group:read");
        assert_eq!(collapsed[0].status, "running");
        assert!(collapsed[0].markdown.is_empty());
        assert_eq!(collapsed[1].id, "reply");
        assert!(collapsed[1].can_branch);
        let expanded =
            TimelineModule::rows(&session, |id| id == "process-group:read" || id == "search");
        assert_eq!(
            expanded
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["process-group:read", "read", "search", "reply"]
        );
        assert!(!expanded[1].expanded);
        assert!(expanded[2].expanded && expanded[2].can_copy);
        assert_eq!(session.timeline.len(), 3);
        assert!(session.timeline_has_more_before);
    }
    #[test]
    fn failed_retryable_events_stay_visible_when_process_group_is_collapsed() {
        let mut failure = event("failure", "tool", "error");
        failure.can_retry = true;
        let session = session(vec![
            event("read", "tool", "success"),
            event("search", "tool", "success"),
            failure,
        ]);
        let rows = TimelineModule::rows(&session, |_| false);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].id, "failure");
        assert!(rows[1].can_retry);
        assert_eq!(rows[1].status, "error");
    }
    #[test]
    fn history_attachments_survive_projection_and_resolve_by_message_identity() {
        let mut message = event("sent-message", "message", "completed");
        message.message_role = Some("user".into());
        message.markdown = None;
        message.attachments = vec![crate::application::ChatAttachment {
            id: "capture".into(),
            name: "capture.png".into(),
            path: "/history/capture.png".into(),
            kind: crate::application::ChatAttachmentKind::File,
            size: Some(42),
            exists: true,
            mime: Some("image/png".into()),
            directory: None,
        }];
        let mut other = message.clone();
        other.id = "other-message".into();
        other.attachments[0].path = "/other/capture.png".into();
        let session = session(vec![message, other]);
        let rows = TimelineModule::rows(&session, |_| false);
        assert_eq!(rows[0].attachments, session.timeline[0].attachments);
        assert_eq!(
            session
                .timeline_attachment(&rows[0].id, "capture")
                .unwrap()
                .path,
            "/history/capture.png"
        );
        assert_eq!(
            session
                .timeline_attachment(&rows[1].id, "capture")
                .unwrap()
                .path,
            "/other/capture.png"
        );
        assert!(session
            .timeline_attachment("not-in-this-task", "capture")
            .is_none());
        assert!(session
            .timeline_attachment("sent-message", "not-attached")
            .is_none());
    }
}
