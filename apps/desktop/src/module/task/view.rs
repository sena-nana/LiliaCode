use crate::module::composer::pending_view::{PendingSnapshot, PendingView};
use crate::module::composer::view::{ComposerView, ComposerViewSnapshot};
use crate::module::timeline::view::{TimelineView, TimelineViewSnapshot};
use crate::runtime_layout::{conversation_headline, headline_slot, reconcile_children};
use crate::runtime_shell::{emit, IntentSink, ShellIntent};
use nana_ui::runtime::view::{entity_ref, widget, with_refs};
use nana_ui::runtime::{AppContext, DocumentId, Entity, FrameworkError, LengthSpec, Stack, Text};
use std::sync::Arc;

pub(crate) const CHAT_CONTENT_MAX_WIDTH: f32 = 760.0;

pub(crate) struct TaskViewInput<'a> {
    pub heading: &'a str,
    pub error: Option<&'a str>,
    pub timeline: &'a TimelineViewSnapshot,
    pub composer: &'a ComposerViewSnapshot,
    pub pending: Option<&'a PendingSnapshot>,
}

pub(crate) struct TaskView {
    pub conversation_column: Entity<Stack>,
    pub conversation_body: Entity<Stack>,
    pub heading_slot: Entity<Stack>,
    pub heading: Entity<Text>,
    pub error: Entity<Text>,
    pub timeline_view: TimelineView,
    pub pending_view: PendingView,
    pub composer_view: ComposerView,
    synced_timeline: Option<TimelineViewSnapshot>,
}

impl TaskView {
    pub fn mount(
        context: &mut AppContext,
        document: DocumentId,
        input: TaskViewInput<'_>,
        sink: IntentSink,
    ) -> Result<Self, FrameworkError> {
        let (_, (conversation_column, conversation_body, heading_slot, heading, error)) =
            context.mount_view_detached(document, move || {
                let conversation_column = entity_ref::<Stack>();
                let conversation_body = entity_ref::<Stack>();
                let heading_slot = entity_ref::<Stack>();
                let heading = entity_ref::<Text>();
                let error = entity_ref::<Text>();
                with_refs(
                    (
                        widget(
                            Stack::fill_column(12.0)
                                .max_width(CHAT_CONTENT_MAX_WIDTH)
                                .with_layout(|layout| {
                                    layout.margin_left = Some(LengthSpec::Auto);
                                    layout.margin_right = Some(LengthSpec::Auto);
                                }),
                        )
                        .entity_ref(conversation_column),
                        widget(Stack::fill_column(12.0))
                            .entity_ref(conversation_body)
                            .children((
                                widget(headline_slot(false)).entity_ref(heading_slot),
                                widget(Text::new(input.error.unwrap_or_default()))
                                    .entity_ref(error),
                            )),
                        widget(conversation_headline(input.heading.to_owned())).entity_ref(heading),
                    ),
                    (
                        conversation_column,
                        conversation_body,
                        heading_slot,
                        heading,
                        error,
                    ),
                )
            })?;
        let timeline_sink = Arc::clone(&sink);
        let timeline_view = TimelineView::mount(
            context,
            document,
            input.timeline.target.clone(),
            Arc::new(move |target, action| {
                emit(
                    &timeline_sink,
                    ShellIntent::AddressedTimeline { target, action },
                )
            }),
        )?;
        let pending_sink = Arc::clone(&sink);
        let pending_view = PendingView::mount(
            context,
            document,
            Arc::new(move |target, action| {
                emit(
                    &pending_sink,
                    ShellIntent::AddressedPending { target, action },
                )
            }),
        )?;
        let composer_view = ComposerView::mount(context, document, input.composer, sink)?;
        context.append_child(conversation_body, heading_slot)?;
        context.append_child(conversation_body, error)?;
        context.append_child(conversation_body, timeline_view.root)?;
        let mut view = Self {
            conversation_column,
            conversation_body,
            heading_slot,
            heading,
            error,
            timeline_view,
            pending_view,
            composer_view,
            synced_timeline: None,
        };
        view.sync(context, document, input)?;
        Ok(view)
    }

    pub fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        input: TaskViewInput<'_>,
    ) -> Result<(), FrameworkError> {
        let headline_active = !input.heading.trim().is_empty();
        context.update_component(self.heading, |heading, _| {
            *heading = conversation_headline(input.heading.to_owned())
        })?;
        context.update_component(self.heading_slot, |slot, _| {
            *slot = headline_slot(headline_active)
        })?;
        let headline = headline_active
            .then_some(self.heading.stable_id())
            .into_iter()
            .collect::<Vec<_>>();
        reconcile_children(context, self.heading_slot.stable_id(), &headline)?;
        let body = if headline_active {
            vec![self.heading_slot.stable_id(), self.error.stable_id()]
        } else {
            vec![self.error.stable_id(), self.timeline_view.root.stable_id()]
        };
        reconcile_children(context, self.conversation_body.stable_id(), &body)?;
        context.update_component(self.error, |error, _| {
            *error = Text::new(input.error.unwrap_or_default())
        })?;
        if self.synced_timeline.as_ref() != Some(input.timeline) {
            self.timeline_view.sync(context, document, input.timeline)?;
            self.synced_timeline = Some(input.timeline.clone());
        }
        self.composer_view.sync(context, document, input.composer)?;
        self.pending_view.sync(
            context,
            document,
            input.composer.window_id,
            input.timeline.target.task_id.clone(),
            input.pending,
        )?;
        let mut children = vec![self.conversation_body.stable_id()];
        if input.pending.is_some() {
            children.push(self.pending_view.root.stable_id());
        } else {
            // ComposerView owns a single stage so the card and its external
            // compact controls are reconciled atomically.
            children.push(self.composer_view.stage.stable_id());
        }
        reconcile_children(context, self.conversation_column.stable_id(), &children)?;
        self.pending_view.restore_focus(context, document)
    }
}
