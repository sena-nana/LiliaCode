//! Timeline view state as a UI module.
//!
//! The events themselves live on the task session the shell publishes. This
//! module owns expansion state for its window.

pub mod view;

use std::collections::{BTreeMap, BTreeSet};

use lilia_contracts::{TaskId, TimelineProjectionCursor};

use lilia_kernel::FeatureId;

use self::view::{StepKind, TimelineRole, TimelineRow, TimelineTone};
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
    TextSelected {
        event_id: String,
        text: Option<String>,
    },
    ClearTextSelection,
    SearchChanged(String),
    SearchStep(isize),
    SearchOpen(bool),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TimelineReadingPosition {
    pub offset: f32,
    pub extent: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TimelineReadingAnchor {
    pub position: TimelineReadingPosition,
    pub oldest: Option<TimelineProjectionCursor>,
    pub at_end: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TimelineMessageHit {
    pub event_id: String,
    pub sequence: u64,
}

struct TimelineMessageSearch {
    query: String,
    hits: Vec<TimelineMessageHit>,
    active: usize,
}

pub struct TimelineModule {
    reading_anchors: BTreeMap<TaskId, TimelineReadingAnchor>,
    message_searches: BTreeMap<TaskId, TimelineMessageSearch>,
    searching: BTreeSet<TaskId>,
    toggled_events: BTreeSet<String>,
    text_selection: Option<TimelineTextSelection>,
}

impl Default for TimelineModule {
    fn default() -> Self {
        Self {
            reading_anchors: BTreeMap::new(),
            message_searches: BTreeMap::new(),
            searching: BTreeSet::new(),
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
                let expanded = self.toggled_events.contains(&group_id);
                let children = &session.timeline[index..end];
                rows.push(Self::group_row(group_id, children, expanded));
                if expanded {
                    for child in children {
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
        let previous = self.reading_anchors.get(&task_id);
        let mut oldest = previous.and_then(|anchor| anchor.oldest.clone());
        let mut at_end = previous.is_some_and(|anchor| anchor.at_end);
        if let Some(session) = session {
            oldest = session
                .timeline
                .first()
                .map(|event| TimelineProjectionCursor {
                    sequence: event.sequence,
                    event_id: event.id.clone(),
                });
            at_end = reading_position_is_at_end(position, &session);
        }
        self.reading_anchors.insert(
            task_id,
            TimelineReadingAnchor {
                position,
                oldest,
                at_end,
            },
        );
    }

    pub(crate) fn reading_position(&self, task_id: &TaskId) -> Option<TimelineReadingPosition> {
        self.reading_anchors
            .get(task_id)
            .map(|anchor| anchor.position)
    }

    pub(crate) fn reading_anchor(&self, task_id: &TaskId) -> Option<&TimelineReadingAnchor> {
        self.reading_anchors.get(task_id)
    }

    pub(crate) fn apply_message_search(
        &mut self,
        task_id: TaskId,
        query: String,
        hits: Vec<TimelineMessageHit>,
    ) {
        let previous_event = self
            .message_searches
            .get(&task_id)
            .filter(|search| search.query == query)
            .and_then(|search| search.hits.get(search.active))
            .map(|hit| hit.event_id.clone());
        let active = previous_event
            .and_then(|event_id| hits.iter().position(|hit| hit.event_id == event_id))
            .unwrap_or_else(|| hits.len().saturating_sub(1));
        self.message_searches.insert(
            task_id,
            TimelineMessageSearch {
                query,
                hits,
                active,
            },
        );
    }

    pub(crate) fn step_message_search(
        &mut self,
        task_id: &TaskId,
        delta: isize,
    ) -> Option<TimelineMessageHit> {
        let search = self.message_searches.get_mut(task_id)?;
        let len = search.hits.len() as isize;
        if len == 0 {
            return None;
        }
        search.active = (search.active as isize + delta).rem_euclid(len) as usize;
        search.hits.get(search.active).cloned()
    }

    pub(crate) fn active_message_hit(&self, task_id: &TaskId) -> Option<TimelineMessageHit> {
        let search = self.message_searches.get(task_id)?;
        if search.query.trim().is_empty() {
            return None;
        }
        search.hits.get(search.active).cloned()
    }

    pub(crate) fn search_open(&self, task_id: Option<&TaskId>) -> bool {
        task_id.is_some_and(|task_id| {
            self.searching.contains(task_id)
                || self
                    .message_searches
                    .get(task_id)
                    .is_some_and(|search| !search.query.is_empty())
        })
    }

    pub(crate) fn message_search_view(&self, task_id: Option<&TaskId>) -> (String, String, bool) {
        let Some(search) = task_id.and_then(|task_id| self.message_searches.get(task_id)) else {
            return (String::new(), String::new(), false);
        };
        let status = if search.query.trim().is_empty() {
            String::new()
        } else if search.hits.is_empty() {
            "没有匹配".to_owned()
        } else {
            format!("{}/{}", search.active + 1, search.hits.len())
        };
        (search.query.clone(), status, !search.hits.is_empty())
    }

    pub fn feature_id() -> FeatureId {
        FeatureId::new("lilia.timeline").expect("the timeline feature id is not blank")
    }

    pub fn text_selection(&self) -> Option<&TimelineTextSelection> {
        self.text_selection.as_ref()
    }

    /// Records what a message reports as selected; returns whether that
    /// changed the selection.
    fn select_text(&mut self, event_id: String, text: Option<String>) -> bool {
        match text.filter(|text| !text.trim().is_empty()) {
            Some(text) => {
                self.text_selection = Some(TimelineTextSelection { event_id, text });
                true
            }
            // Selecting in one message clears the one before; that late
            // "cleared" must not drop the new selection.
            None if self
                .text_selection
                .as_ref()
                .is_some_and(|selection| selection.event_id == event_id) =>
            {
                self.text_selection = None;
                true
            }
            None => false,
        }
    }

    fn group_row(id: String, children: &[TaskTimelineItem], expanded: bool) -> TimelineRow {
        let mut counts: Vec<(StepKind, usize)> = Vec::new();
        let mut tone = TimelineTone::Settled;
        for child in children {
            let kind = StepKind::classify(&child.kind, child.tool.as_deref(), &child.status);
            match counts.iter_mut().find(|(seen, _)| *seen == kind) {
                Some((_, count)) => *count += 1,
                None => counts.push((kind, 1)),
            }
            if TimelineTone::from_status(&child.status) == TimelineTone::Running {
                tone = TimelineTone::Running;
            }
        }
        let detail = counts
            .iter()
            .take(3)
            .map(|(kind, count)| {
                if *count > 1 {
                    format!("{} ×{count}", kind.label())
                } else {
                    kind.label().to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(" · ");
        TimelineRow {
            id,
            role: TimelineRole::Group,
            tone,
            title: format!("执行过程 · {} 步", children.len()),
            detail,
            markdown: String::new(),
            images: Vec::new(),
            expanded,
            can_expand: true,
            can_retry: false,
            can_copy: false,
            can_branch: false,
            key_node: false,
        }
    }

    pub fn row(
        item: &TaskTimelineItem,
        toggled: bool,
        can_retry: bool,
        can_branch: bool,
    ) -> TimelineRow {
        let role = match item.message_role.as_deref() {
            Some("user") => TimelineRole::User,
            Some("assistant") => TimelineRole::Reply,
            _ => TimelineRole::Step(StepKind::classify(
                &item.kind,
                item.tool.as_deref(),
                &item.status,
            )),
        };
        let full = item
            .markdown
            .clone()
            .or_else(|| item.summary.clone())
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| item.title.clone());
        let can_copy = item
            .markdown_plain_text
            .as_ref()
            .or(item.markdown.as_ref())
            .is_some_and(|text| !text.trim().is_empty());
        let TimelineRole::Step(kind) = role else {
            return TimelineRow {
                id: item.id.clone(),
                role,
                tone: TimelineTone::from_status(&item.status),
                title: String::new(),
                detail: String::new(),
                markdown: full,
                images: Vec::new(),
                expanded: true,
                can_expand: false,
                can_retry,
                can_copy,
                can_branch,
                key_node: true,
            };
        };
        let summary = item
            .summary
            .as_deref()
            .map(str::trim)
            .filter(|summary| !summary.is_empty());
        let title = match kind {
            StepKind::Status => summary.unwrap_or(kind.label()).to_owned(),
            _ => kind.label().to_owned(),
        };
        let detail = match kind {
            StepKind::Status => String::new(),
            _ => summary
                .or(item.markdown.as_deref())
                .map(first_line)
                .filter(|line| *line != title)
                .unwrap_or_default()
                .to_owned(),
        };
        let body = item
            .markdown
            .clone()
            .filter(|text| !text.trim().is_empty())
            .or_else(|| {
                summary
                    .filter(|text| *text != detail || text.contains('\n'))
                    .map(str::to_owned)
            })
            .unwrap_or_default();
        let can_expand = !body.trim().is_empty() && (body.trim() != detail || body.contains('\n'));
        TimelineRow {
            id: item.id.clone(),
            role,
            tone: TimelineTone::from_status(&item.status),
            title,
            detail,
            markdown: if can_expand { body } else { String::new() },
            images: Vec::new(),
            expanded: toggled,
            can_expand,
            can_retry,
            can_copy: false,
            can_branch: false,
            key_node: false,
        }
    }
}

fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
}

const TIMELINE_LOAD_EARLIER_EXTENT: f32 = 34.0;
const TIMELINE_TAIL_TOLERANCE: f32 = 24.0;

fn reading_position_is_at_end(
    position: TimelineReadingPosition,
    session: &TaskSessionView,
) -> bool {
    let content = session.timeline_layout.total_extent()
        + if session.timeline_has_more_before {
            TIMELINE_LOAD_EARLIER_EXTENT
        } else {
            0.0
        };
    let maximum_offset = (content - position.extent).max(0.0);
    position.offset >= (maximum_offset - TIMELINE_TAIL_TOLERANCE).max(0.0)
}

pub(crate) fn timeline_reaches_cursor(
    session: &TaskSessionView,
    cursor: &TimelineProjectionCursor,
) -> bool {
    if session
        .timeline
        .iter()
        .any(|event| event.id == cursor.event_id)
    {
        return true;
    }
    let Some(first) = session.timeline.first() else {
        return false;
    };
    first.sequence < cursor.sequence
        || (first.sequence == cursor.sequence && first.id.as_str() <= cursor.event_id.as_str())
}

fn project_message_search(
    module: &TimelineModule,
    cx: &UiModuleContext<'_>,
    timeline: &mut crate::module::timeline::view::TimelineViewSnapshot,
) {
    let (query, status, can_step) = module.message_search_view(cx.selected_task().as_ref());
    timeline.search_open = module.search_open(cx.selected_task().as_ref());
    timeline.selection = module
        .text_selection()
        .map(|selection| selection.event_id.clone());
    timeline.search_query = query;
    timeline.search_status = status;
    timeline.search_can_step = can_step;
}

fn message_hits(
    cx: &UiModuleContext<'_>,
    task_id: &TaskId,
    query: &str,
) -> Vec<TimelineMessageHit> {
    cx.kernel()
        .service::<lilia_feature_timeline::TimelineServiceKey>()
        .map(|service| {
            crate::application::search_timeline_bodies(query, &service.events(task_id), 100)
                .into_iter()
                .map(|hit| TimelineMessageHit {
                    event_id: hit.event_id,
                    sequence: hit.sequence,
                })
                .collect()
        })
        .unwrap_or_default()
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

    fn resync(&mut self, _cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        UiModuleOutcome::dirty()
    }

    fn reduce(&mut self, message: Self::Message, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        match message {
            TimelineModuleMessage::Toggle(event_id) => {
                if !self.toggled_events.remove(&event_id) {
                    self.toggled_events.insert(event_id);
                }
                UiModuleOutcome::dirty()
            }
            TimelineModuleMessage::TextSelected { event_id, text } => {
                if self.select_text(event_id, text) {
                    UiModuleOutcome::dirty()
                } else {
                    UiModuleOutcome::clean()
                }
            }
            TimelineModuleMessage::ClearTextSelection => {
                self.text_selection = None;
                UiModuleOutcome::dirty()
            }
            TimelineModuleMessage::SearchChanged(query) => {
                let Some(task_id) = cx.selected_task() else {
                    return UiModuleOutcome::clean();
                };
                let hits = message_hits(cx, &task_id, &query);
                self.apply_message_search(task_id, query, hits);
                UiModuleOutcome::dirty()
            }
            TimelineModuleMessage::SearchOpen(open) => {
                let Some(task_id) = cx.selected_task() else {
                    return UiModuleOutcome::clean();
                };
                if open {
                    self.searching.insert(task_id);
                } else {
                    self.searching.remove(&task_id);
                    self.message_searches.remove(&task_id);
                }
                UiModuleOutcome::dirty()
            }
            TimelineModuleMessage::SearchStep(delta) => {
                let Some(task_id) = cx.selected_task() else {
                    return UiModuleOutcome::clean();
                };
                self.step_message_search(&task_id, delta);
                UiModuleOutcome::dirty()
            }
        }
    }

    fn project_fields(&self, cx: &UiModuleContext<'_>, into: Self::Projection<'_>) {
        if !crate::module::conversation_is_visible(cx) {
            return;
        }
        project_message_search(self, cx, into.timeline);
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
        {
            if let Some(task_id) = selected.clone() {
                if let Some(query) = self
                    .message_searches
                    .get(&task_id)
                    .map(|search| search.query.clone())
                    .filter(|query| !query.trim().is_empty())
                {
                    let hits = message_hits(cx, &task_id, &query);
                    self.apply_message_search(task_id, query, hits);
                }
            }
            return UiModuleOutcome::dirty();
        }
        if envelope
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
    fn a_selection_moving_between_messages_survives_the_old_one_clearing() {
        let mut module = TimelineModule::default();
        assert!(!module.select_text("reply-1".into(), Some("  ".into())));
        assert!(module.text_selection().is_none());

        assert!(module.select_text("reply-1".into(), Some("first".into())));
        assert!(module.select_text("reply-2".into(), Some("second".into())));
        assert!(!module.select_text("reply-1".into(), None));
        let selection = module.text_selection().unwrap();
        assert_eq!(
            (selection.event_id.as_str(), selection.text.as_str()),
            ("reply-2", "second")
        );

        assert!(module.select_text("reply-2".into(), None));
        assert!(module.text_selection().is_none());
    }

    #[test]
    fn conversation_body_is_visible_and_process_details_are_opt_in() {
        let mut item = TaskTimelineItem {
            id: "reply".into(),
            sequence: 1,
            turn_id: None,
            kind: "message".into(),
            tool: None,
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
        let reply = TimelineModule::row(&item, false, false, false);
        assert_eq!(reply.role, TimelineRole::Reply);
        assert_eq!(reply.markdown, "Complete response");
        assert!(!reply.can_expand);
        item.message_role = Some("user".into());
        assert_eq!(
            TimelineModule::row(&item, false, false, false).role,
            TimelineRole::User
        );
        item.message_role = None;
        item.kind = "tool_call".into();
        item.tool = Some("read_file".into());
        let collapsed = TimelineModule::row(&item, false, false, false);
        assert_eq!(collapsed.role, TimelineRole::Step(StepKind::Read));
        assert_eq!(collapsed.title, "读取");
        assert_eq!(collapsed.detail, "Summary");
        assert!(collapsed.can_expand && !collapsed.expanded);
        let expanded = TimelineModule::row(&item, true, false, false);
        assert!(expanded.expanded);
        assert_eq!(expanded.markdown, "Complete response");
    }

    fn process_item(id: &str, kind: &str, status: &str) -> TaskTimelineItem {
        TaskTimelineItem {
            id: id.into(),
            sequence: 1,
            turn_id: Some("turn".into()),
            kind: kind.into(),
            tool: None,
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
        assert_eq!(collapsed[0].role, TimelineRole::Group);
        assert_eq!(collapsed[0].title, "执行过程 · 2 步");
        assert_eq!(collapsed[0].tone, TimelineTone::Running);
        assert_eq!(collapsed[1].id, "reply");
        assert!(!collapsed[0].key_node);
        assert!(collapsed[1].key_node);
        let mut expanded = TimelineModule::default();
        expanded.toggled_events.insert("process-group:read".into());
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
        assert!(TimelineModule::default()
            .reading_position(&task_a)
            .is_none());
    }

    #[test]
    fn reading_anchors_keep_the_oldest_cursor_past_eight_tasks() {
        let mut timeline = TimelineModule::default();
        let mut first_task = None;
        for index in 0..9 {
            let task_id = TaskId::new(format!("task-{index}")).unwrap();
            let mut event = process_item(&format!("event-{index}"), "message", "completed");
            event.sequence = 20 + index as u64;
            let session = process_session(vec![event]);
            timeline.remember_reading_position(
                task_id.clone(),
                TimelineReadingPosition {
                    offset: index as f32 * 40.0,
                    extent: 200.0,
                },
                Some(session),
            );
            if index == 0 {
                first_task = Some(task_id);
            }
        }
        let first_task = first_task.unwrap();
        let anchor = timeline.reading_anchor(&first_task).unwrap();
        assert_eq!(anchor.position.offset, 0.0);
        assert_eq!(anchor.oldest.as_ref().unwrap().event_id, "event-0");
        assert_eq!(anchor.oldest.as_ref().unwrap().sequence, 20);
        let mut later = process_session(vec![process_item("late", "message", "completed")]);
        later.timeline[0].sequence = 80;
        assert!(!timeline_reaches_cursor(
            &later,
            anchor.oldest.as_ref().unwrap()
        ));
        later
            .timeline
            .insert(0, process_item("event-0", "message", "completed"));
        later.timeline[0].sequence = 20;
        assert!(timeline_reaches_cursor(
            &later,
            anchor.oldest.as_ref().unwrap()
        ));
    }

    #[test]
    fn message_search_steps_wrap_around_hits_without_touching_turn_state() {
        let task_id = TaskId::new("task-search").unwrap();
        let mut timeline = TimelineModule::default();
        let hits = vec![
            TimelineMessageHit {
                event_id: "early".into(),
                sequence: 2,
            },
            TimelineMessageHit {
                event_id: "middle".into(),
                sequence: 5,
            },
            TimelineMessageHit {
                event_id: "latest".into(),
                sequence: 9,
            },
        ];
        timeline.apply_message_search(task_id.clone(), "登录".into(), hits);
        assert_eq!(
            timeline.active_message_hit(&task_id).unwrap().event_id,
            "latest"
        );
        timeline.step_message_search(&task_id, -1);
        assert_eq!(
            timeline.active_message_hit(&task_id).unwrap().event_id,
            "middle"
        );
        timeline.step_message_search(&task_id, 1);
        assert_eq!(
            timeline.active_message_hit(&task_id).unwrap().event_id,
            "latest"
        );
        timeline.step_message_search(&task_id, 1);
        assert_eq!(
            timeline.active_message_hit(&task_id).unwrap().event_id,
            "early"
        );
        timeline.apply_message_search(task_id.clone(), "   ".into(), Vec::new());
        assert!(timeline.active_message_hit(&task_id).is_none());
    }
}
