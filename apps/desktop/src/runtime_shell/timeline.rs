//! The main timeline uses one coordinate space for placement, measurement and scroll.
//! Geometry is sampled only after presentation, never while installing a new row.
use super::*;
use nana_ui::runtime::{RuntimeDocument, VirtualViewport};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimelinePresentation {
    pub offset: f32,
    pub viewport_extent: f32,
    pub content_extent: f32,
    pub redraw: bool,
}

#[derive(Default)]
pub(super) struct TimelineMeasurements {
    pub(super) task: Option<String>,
    rows: Vec<ShellTimelineRow>,
    indices: HashMap<String, usize>,
    estimates: VirtualListLayout,
    layout: VirtualListLayout,
    measured: HashMap<String, f32>,
    width: Option<f32>,
    viewport: f32,
    queued_tail_task: Option<Option<String>>,
    offset: f32,
    pending_offset: Option<f32>,
    pending_tail: bool,
    tail_task: Option<String>,
    pending_observed_offset: Option<f32>,
    reported: Option<(f32, f32, f32)>,
    presented_content_extent: Option<f32>,
}

impl TimelineMeasurements {
    fn content_extent(&self) -> f32 {
        self.layout.total_extent()
    }

    fn cancel_superseded_correction(&mut self, actual: f32, max: f32) {
        if self
            .pending_observed_offset
            .is_some_and(|old| (old.min(max) - actual).abs() > 0.5)
        {
            self.pending_offset = None;
            self.pending_observed_offset = None;
            self.pending_tail = false;
        }
    }
    pub(super) fn anchor(&self, offset: f32) -> Option<(String, f32)> {
        let anchor = self.layout.scroll_anchor(offset)?;
        Some((self.rows.get(anchor.index)?.id.clone(), anchor.inset))
    }

    fn anchored_offset(&self, anchor: Option<(String, f32)>, fallback: f32) -> f32 {
        if self.pending_tail {
            return (self.content_extent() - self.viewport).max(0.0);
        }
        anchor
            .and_then(|(key, inset)| {
                self.indices.get(&key).map(|&index| {
                    (self.layout.extent(0..index) + inset.min(self.layout.extent(index..index + 1)))
                        .min((self.content_extent() - self.viewport).max(0.0))
                })
            })
            .unwrap_or(fallback.min((self.content_extent() - self.viewport).max(0.0)))
    }
}

impl ShellHandles {
    pub(super) fn prepare_timeline_projection(
        &mut self,
        context: &AppContext,
        snapshot: &PrimaryShellSnapshot,
    ) {
        let viewport = timeline_viewport_extent(context, self.timeline_scroll, snapshot);
        let actual = context
            .world()
            .scroll_metrics(self.timeline_scroll.stable_id())
            .and_then(|_| {
                context
                    .world()
                    .scroll_offset(self.timeline_scroll.stable_id())
            })
            .map(|v| v.y);
        let state = &mut self.timeline_measurements;
        let task_changed = state.task != snapshot.composer_task_id;
        if let Some(actual) = actual {
            let max = context
                .world()
                .scroll_metrics(self.timeline_scroll.stable_id())
                .map(|metrics| metrics.max_offset().y)
                .unwrap_or_default();
            state.cancel_superseded_correction(actual, max);
        }
        let offset = if task_changed {
            snapshot.timeline_scroll_offset.max(0.0)
        } else {
            state
                .pending_offset
                .or(actual)
                .unwrap_or(snapshot.timeline_scroll_offset.max(0.0))
        };
        let anchor = state.anchor(offset);
        let mut rows = snapshot.timeline.clone();
        if snapshot.can_interrupt || snapshot.pending_blocks_send {
            for row in &mut rows {
                row.can_branch = false;
                row.can_apply = false;
            }
        }
        if task_changed || state.rows != rows || state.estimates != snapshot.timeline_layout {
            if task_changed {
                state.pending_tail &= state.tail_task == snapshot.composer_task_id;
                state.pending_offset = None;
                state.pending_observed_offset = None;
                state.reported = None;
                state.presented_content_extent = None;
                state.measured.clear();
                state.width = None;
            } else {
                let current = rows
                    .iter()
                    .map(|row| (&row.id, row))
                    .collect::<HashMap<_, _>>();
                state.measured.retain(|key, _| {
                    let old = state
                        .indices
                        .get(key)
                        .and_then(|&index| state.rows.get(index));
                    current.get(key).copied() == old
                });
            }
            state.task = snapshot.composer_task_id.clone();
            if state.queued_tail_task.as_ref() == Some(&state.task) {
                state.queued_tail_task = None;
                state.pending_tail = true;
                state.tail_task = state.task.clone();
            }
            state.rows = rows;
            state.indices = state
                .rows
                .iter()
                .enumerate()
                .map(|(index, row)| (row.id.clone(), index))
                .collect();
            state.estimates = timeline_virtual_layout(snapshot);
            state.layout = state.estimates.clone();
            for (key, &height) in &state.measured {
                if let Some(&index) = state.indices.get(key) {
                    state.layout.update_item_extent(index, height);
                }
            }
            state.viewport = viewport;
            state.offset = state.anchored_offset(if task_changed { None } else { anchor }, offset);
            state.pending_offset = Some(state.offset);
            state.pending_observed_offset = actual;
        } else {
            state.viewport = viewport;
            state.offset = if state.pending_tail {
                (state.content_extent() - viewport).max(0.0)
            } else {
                offset
            };
        }
    }

    pub(super) fn materialize_timeline(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
    ) -> Result<(), FrameworkError> {
        let state = &self.timeline_measurements;
        context.materialize_virtual_list_retained_in(
            self.timeline_list,
            &mut self.timeline_virtual,
            &state.layout,
            VirtualViewport::vertical(state.offset, state.viewport, TIMELINE_OVERSCAN_EXTENT),
            &[],
            |index| state.rows[index].id.clone(),
            |key| state.indices.get(key).copied(),
            |_, _| Stack::fill_column(6.0),
        )?;
        let mounted = self.timeline_virtual.mounted_keys().to_vec();
        let keep = mounted.iter().collect::<HashSet<_>>();
        let released = self
            .timeline_content
            .keys()
            .filter(|key| !keep.contains(key))
            .cloned()
            .collect::<Vec<_>>();
        for key in released {
            if let Some(content) = self.timeline_content.remove(&key) {
                content.dispose_owned(context)?;
            }
        }
        for key in mounted {
            let row = &state.rows[state.indices[&key]];
            let root = self
                .timeline_virtual
                .entity(&key)
                .expect("materialized timeline row");
            if !self.timeline_content.contains_key(&key) {
                self.timeline_content.insert(
                    key.clone(),
                    crate::runtime_conversation::TimelineContent::mount(context, document_id)?,
                );
            }
            self.timeline_content
                .get_mut(&key)
                .expect("timeline content")
                .sync(
                    context,
                    document_id,
                    root,
                    nana_ui_platform::WindowId::PRIMARY,
                    row,
                    &self.sink,
                )?;
            // A placement wrapper carries the estimate; the content must retain
            // its natural height so the following presentation can measure it.
            context.update_component(root, |stack, _| {
                *stack = stack
                    .clone()
                    .with_layout(|layout| layout.flex_shrink = Some(0.0));
            })?;
        }
        Ok(())
    }

    /// Last projected tree only; callers with a newer session must submit a tail
    /// intent rather than treating this as that session's content extent.
    pub fn measured_timeline_content_extent(&self) -> Option<f32> {
        self.timeline_measurements.presented_content_extent
    }

    pub fn measured_timeline_content_extent_for_task(&self, task: Option<&str>) -> Option<f32> {
        (self.timeline_measurements.task.as_deref() == task)
            .then(|| self.measured_timeline_content_extent())
            .flatten()
    }

    /// Preserve the intent through the next sync and measured-layout correction.
    #[cfg(test)]
    pub fn scroll_timeline_to_end(
        &mut self,
        document: &mut RuntimeDocument,
    ) -> Result<bool, FrameworkError> {
        let task = self.timeline_measurements.task.clone();
        self.scroll_timeline_to_end_for_task(document, task.as_deref())
    }

    pub fn scroll_timeline_to_end_for_task(
        &mut self,
        document: &mut RuntimeDocument,
        task: Option<&str>,
    ) -> Result<bool, FrameworkError> {
        let state = &mut self.timeline_measurements;
        // A different projection must not consume this request in its present callback.
        if state.task.as_deref() != task {
            state.queued_tail_task = Some(task.map(str::to_owned));
            return Ok(true);
        }
        state.pending_tail = true;
        state.tail_task = task.map(str::to_owned);
        state.pending_observed_offset = None;
        state.offset = (state.content_extent() - state.viewport).max(0.0);
        state.pending_offset = Some(state.offset);
        let id = document.document();
        self.materialize_timeline(document.context_mut(), id)?;
        Ok(true)
    }

    pub fn on_timeline_presented(
        &mut self,
        document: &mut RuntimeDocument,
    ) -> Result<Option<TimelinePresentation>, FrameworkError> {
        let id = document.document();
        let context = document.context_mut();
        if !context.world().is_mounted(self.timeline_scroll.stable_id()) {
            return Ok(None);
        }
        let Some(bounds) = context.world().layout_box(self.timeline_list.stable_id()) else {
            return Ok(None);
        };
        let Some(metrics) = context
            .world()
            .scroll_metrics(self.timeline_scroll.stable_id())
        else {
            return Ok(None);
        };
        if bounds.width <= 0.0 || metrics.viewport_height <= 0.0 {
            return Ok(None);
        }
        let mut actual = context
            .world()
            .scroll_offset(self.timeline_scroll.stable_id())
            .unwrap_or_default()
            .y;
        let state = &mut self.timeline_measurements;
        let mut redraw = false;
        if let Some(target) = state.pending_offset.take() {
            let expected = state
                .pending_observed_offset
                .take()
                .map(|old| old.min(metrics.max_offset().y));
            let user_scrolled = expected.is_some_and(|expected| (expected - actual).abs() > 0.5);
            if user_scrolled {
                state.pending_tail = false;
            } else {
                redraw |=
                    context.scroll_to(self.timeline_scroll, ScrollOffset { x: 0.0, y: target })?;
                actual = context
                    .world()
                    .scroll_offset(self.timeline_scroll.stable_id())
                    .unwrap_or_default()
                    .y;
            }
        }
        let old_offset = state.offset;
        let old_viewport = state.viewport;
        let anchor = if state.width.is_none() || actual == 0.0 {
            None
        } else {
            state.anchor(actual)
        };
        let was_at_end = state.reported.is_some_and(|(offset, viewport, content)| {
            (offset - (content - viewport).max(0.0)).abs() <= 1.0
                && (actual - offset.min(metrics.max_offset().y)).abs() <= 0.5
        });
        let at_end = state.pending_tail
            || was_at_end
            || (actual - (metrics.content_height - metrics.viewport_height).max(0.0)).abs() <= 1.0;
        let width_changed = state.width != Some(bounds.width);
        let mut changed = false;
        if width_changed {
            state.measured.clear();
            changed = state.layout != state.estimates;
            state.layout = state.estimates.clone();
            state.width = Some(bounds.width);
        }
        state.viewport = metrics.viewport_height;
        // Only installed rows are visited. The host calls this after their new
        // content and containing width have actually completed layout/present.
        for key in self.timeline_virtual.mounted_keys() {
            let Some(root) = self.timeline_virtual.entity(key) else {
                continue;
            };
            let Some(row) = context.world().layout_box(root.stable_id()) else {
                continue;
            };
            if row.height.is_finite() && row.height > 0.0 {
                let index = state.indices[key];
                changed |= state.layout.update_item_extent(index, row.height);
                state.measured.insert(key.clone(), row.height);
            }
        }
        let desired = if at_end {
            (state.content_extent() - state.viewport).max(0.0)
        } else {
            state.anchored_offset(anchor, actual)
        };
        state.offset = desired;
        let rewindow = changed
            || width_changed
            || (old_offset - desired).abs() > 0.5
            || (old_viewport - state.viewport).abs() > 0.5;
        if changed || (desired - actual).abs() > 0.5 {
            state.pending_offset = Some(desired);
            state.pending_observed_offset = Some(actual);
            // Keep tail following only while correction is outstanding. A
            // stable presentation releases it so subsequent user upscroll wins.
            state.pending_tail = at_end;
            state.tail_task = state.task.clone();
        } else {
            state.pending_tail = false;
        }
        // The public query represents the full scrollport, including any
        // structural padding. A corrected list is reported after its next layout.
        state.presented_content_extent = Some(metrics.content_height);
        let values = (desired, state.viewport, metrics.content_height);
        let report_changed = state.reported != Some(values);
        state.reported = Some(values);
        if rewindow {
            self.materialize_timeline(context, id)?;
            redraw = true;
        }
        if report_changed || redraw {
            Ok(Some(TimelinePresentation {
                offset: values.0,
                viewport_extent: values.1,
                content_extent: values.2,
                redraw,
            }))
        } else {
            Ok(None)
        }
    }
}
