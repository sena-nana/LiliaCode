//! Timeline view state as a UI module.
//!
//! The events themselves live on the task session the shell publishes. This
//! module owns expansion state for its window.

pub mod view;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use lilia_contracts::TaskId;

use lilia_kernel::FeatureId;

use self::view::TimelineRow;
use crate::task_session::{TaskSessionView, TaskTimelineItem};
use crate::ui_module::{UiModule, UiModuleContext, UiModuleOutcome};

#[derive(Debug, Clone)]
pub struct TimelineTextSelection {
    pub event_id: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum TimelineModuleMessage {
    Toggle(String),
    ClearTextSelection,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TimelineReadingPosition {
    pub offset: f32,
    pub extent: f32,
}

pub struct TimelineModule {
    reading_positions: BTreeMap<TaskId, TimelineReadingPosition>,
    reading_histories: VecDeque<(TaskId, TaskSessionView)>,
    toggled_events: BTreeSet<String>,
    text_selection: Option<TimelineTextSelection>,
}

impl Default for TimelineModule {
    fn default() -> Self {
        Self {
            reading_positions: BTreeMap::new(),
            reading_histories: VecDeque::new(),
            toggled_events: BTreeSet::new(),
            text_selection: None,
        }
    }
}

impl TimelineModule {
    pub(crate) fn rows(&self, session: &TaskSessionView) -> Vec<TimelineRow> {
        let mut rows = Vec::new();
        let mut index = 0;
        while index < session.timeline.len() {
            let item = &session.timeline[index];
            if item.kind == "turn_state"
                && item.turn_id.is_some()
                && session.timeline[index + 1..].iter().any(|later| {
                    later.kind == "turn_state" && later.turn_id == item.turn_id
                })
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
                let expanded = self.toggled_events.contains(&group_id);
                rows.push(TimelineRow {
                    id: group_id,
                    markdown: format!("执行过程 · {} 项", end - index),
                    images: Vec::new(),
                    expanded,
                    can_expand: true,
                    can_retry: false,
                    can_copy: false,
                    can_branch: false,
                });
                if expanded {
                    for child in &session.timeline[index..end] {
                        rows.push(Self::row(
                            child,
                            self.toggled_events.contains(&child.id),
                            false,
                            false,
                        ));
                    }
                }
            } else {
                rows.push(Self::row(
                    item,
                    self.toggled_events.contains(&item.id),
                    item.can_retry && session.run_block.is_none(),
                    item.session_branch_turn_id.is_some() && session.run_block.is_none(),
                ));
            }
            index = end;
        }
        rows
    }

    pub(crate) fn remember_reading_position(
        &mut self,
        task_id: TaskId,
        position: TimelineReadingPosition,
        session: Option<TaskSessionView>,
    ) {
        self.reading_positions.insert(task_id.clone(), position);
        if let Some(session) = session {
            self.reading_histories.retain(|(id, _)| id != &task_id);
            self.reading_histories.push_back((task_id, session));
            while self.reading_histories.len() > 8 {
                self.reading_histories.pop_front();
            }
        }
    }

    pub(crate) fn reading_position(&self, task_id: &TaskId) -> Option<TimelineReadingPosition> {
        self.reading_positions.get(task_id).copied()
    }

    pub(crate) fn reading_history(&self, task_id: &TaskId) -> Option<&TaskSessionView> {
        self.reading_histories
            .iter()
            .find(|(id, _)| id == task_id)
            .map(|(_, session)| session)
    }

    pub fn feature_id() -> FeatureId {
        FeatureId::new("lilia.timeline").expect("the timeline feature id is not blank")
    }

    pub fn text_selection(&self) -> Option<&TimelineTextSelection> {
        self.text_selection.as_ref()
    }

    pub fn row(
        item: &TaskTimelineItem,
        toggled: bool,
        can_retry: bool,
        can_branch: bool,
    ) -> TimelineRow {
        let expanded =
            toggled != matches!(item.message_role.as_deref(), Some("user" | "assistant"));
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
        let can_expand = item.markdown.as_ref().is_some_and(|markdown| {
            item.summary
                .as_ref()
                .is_some_and(|summary| summary != markdown)
                || item.title != *markdown
        });
        TimelineRow {
            id: item.id.clone(),
            markdown: if expanded || !can_expand {
                full
            } else {
                preview
            },
            images: Vec::new(),
            expanded,
            can_expand,
            can_retry,
            can_copy: item
                .markdown_plain_text
                .as_ref()
                .or(item.markdown.as_ref())
                .is_some_and(|text| !text.trim().is_empty()),
            can_branch,
        }
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

impl UiModule for TimelineModule {
    type Projection<'a> = crate::ui_module::projection::TimelineProjection<'a>;

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
        }
    }

    fn project_fields(&self, cx: &UiModuleContext<'_>, into: Self::Projection<'_>) {
        if !crate::module::conversation_is_visible(cx) {
            return;
        }
        let Some(session) = cx.task_session() else {
            into.timeline.rows.clear();
            into.timeline.layout = nana_ui::VirtualListLayout::default();
            into.timeline.can_load_earlier = false;
            return;
        };
        into.timeline.can_load_earlier = session.timeline_has_more_before;
        into.timeline.layout = session.timeline_layout.clone();
        into.timeline.rows = self.rows(&session);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_body_is_visible_and_process_details_are_opt_in() {
        let mut item = TaskTimelineItem {
            id: "reply".into(),
            sequence: 1,
            turn_id: None,
            kind: "message".into(),
            title: "Reply".into(),
            message_role: Some("assistant".into()),
            summary: Some("Summary".into()),
            markdown: Some("Complete response".into()),
            markdown_document: None,
            markdown_plain_text: Some("Complete response".into()),
            markdown_table_count: 0,
            attachments: Vec::new(),
            status: "completed".into(),
            batch_apply: None,
            session_branch_turn_id: None,
            selectable_reply: true,
            can_retry: false,
        };
        assert_eq!(
            TimelineModule::row(&item, false, false, false).markdown,
            "Complete response"
        );
        assert_eq!(
            TimelineModule::row(&item, true, false, false).markdown,
            "Summary"
        );
        item.message_role = Some("user".into());
        assert!(TimelineModule::row(&item, false, false, false).expanded);
        item.message_role = None;
        item.kind = "tool_call".into();
        assert_eq!(
            TimelineModule::row(&item, false, false, false).markdown,
            "Summary"
        );
        assert_eq!(
            TimelineModule::row(&item, true, false, false).markdown,
            "Complete response"
        );
    }

    fn process_item(id: &str, kind: &str, status: &str) -> TaskTimelineItem {
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

    fn process_session(timeline: Vec<TaskTimelineItem>) -> TaskSessionView {
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
    fn process_groups_collapse_until_expanded() {
        let mut reply = process_item("reply", "message", "completed");
        reply.message_role = Some("assistant".into());
        reply.session_branch_turn_id = Some("turn".into());
        let session = process_session(vec![
            process_item("read", "tool", "success"),
            process_item("search", "tool", "running"),
            reply,
        ]);
        let collapsed = TimelineModule::default().rows(&session);
        assert_eq!(collapsed.len(), 2);
        assert_eq!(collapsed[0].id, "process-group:read");
        assert_eq!(collapsed[0].markdown, "执行过程 · 2 项");
        assert_eq!(collapsed[1].id, "reply");
        let mut expanded = TimelineModule::default();
        expanded
            .toggled_events
            .insert("process-group:read".into());
        let rows = expanded.rows(&session);
        assert_eq!(
            rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            ["process-group:read", "read", "search", "reply"]
        );
    }

    #[test]
    fn failed_retryable_events_stay_visible_when_process_group_is_collapsed() {
        let mut failure = process_item("failure", "tool", "error");
        failure.can_retry = true;
        let session = process_session(vec![
            process_item("read", "tool", "success"),
            process_item("search", "tool", "success"),
            failure,
        ]);
        let rows = TimelineModule::default().rows(&session);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].id, "failure");
        assert!(rows[1].can_retry);
    }

    #[test]
    fn switching_tasks_preserves_independent_reading_positions_and_first_visits() {
        let task_a = TaskId::new("task-a").unwrap();
        let task_b = TaskId::new("task-b").unwrap();
        let mut timeline = TimelineModule::default();
        assert!(timeline.reading_position(&task_a).is_none());
        let a = TimelineReadingPosition {
            offset: 120.0,
            extent: 600.0,
        };
        timeline.remember_reading_position(task_a.clone(), a, None);
        assert!(timeline.reading_position(&task_b).is_none());
        let b = TimelineReadingPosition {
            offset: 980.0,
            extent: 400.0,
        };
        timeline.remember_reading_position(task_b.clone(), b, None);
        assert_eq!(timeline.reading_position(&task_a), Some(a));
        assert_eq!(timeline.reading_position(&task_b), Some(b));
        let reread = TimelineReadingPosition { offset: 240.0, ..a };
        timeline.remember_reading_position(task_a.clone(), reread, None);
        assert_eq!(timeline.reading_position(&task_a), Some(reread));
        assert_eq!(timeline.reading_position(&task_b), Some(b));
        assert!(
            TimelineModule::default()
                .reading_position(&task_a)
                .is_none()
        );
    }
}
