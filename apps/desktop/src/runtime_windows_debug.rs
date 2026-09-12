use super::*;
use crate::agent_debug::DebugCommand;
use nana_ui::runtime::StableNodeId;
use serde_json::{json, Value};

impl TaskPopupHandles {
    fn debug_targets(&self) -> [(&'static str, StableNodeId); 2] {
        [
            ("lilia.ui.popup.composer", self.composer.stable_id()),
            ("lilia.ui.popup.send", self.send.stable_id()),
        ]
    }

    fn debug_exposed_point(&self, context: &AppContext, id: StableNodeId) -> Option<(f32, f32)> {
        let mut node = Some(id);
        let mut mounted = false;
        while let Some(current) = node {
            if context
                .world()
                .accessibility(current)
                .is_some_and(|state| state.disabled)
            {
                return None;
            }
            if current == self.shell.stable_id() {
                mounted = true;
                break;
            }
            node = context.world().node(current).and_then(|node| node.parent);
        }
        if !mounted {
            return None;
        }
        let bounds = context.world().layout_box(id)?;
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return None;
        }
        let doc = DocumentId::new(10_000u64.saturating_add(self.window_id.0))?;
        for fy in [0.5, 0.2, 0.8] {
            for fx in [0.5, 0.2, 0.8] {
                let (x, y) = context.world().layout_pointer_position(
                    id,
                    bounds.x + bounds.width * fx,
                    bounds.y + bounds.height * fy,
                )?;
                let mut hit = context.world().hit_test(doc, x, y);
                while let Some(current) = hit {
                    if current == id {
                        return Some((x, y));
                    }
                    hit = context.world().node(current).and_then(|node| node.parent);
                }
            }
        }
        None
    }

    pub(crate) fn debug_retained_command(
        &self,
        document: &mut nana_ui::runtime::RuntimeDocument,
        command: &DebugCommand,
    ) -> Result<Option<Value>, FrameworkError> {
        let doc =
            DocumentId::new(10_000u64.saturating_add(self.window_id.0)).expect("popup document");
        let context = document.context_mut();
        context.rebuild_hit_test(doc);
        if matches!(command, DebugCommand::UiObserve) {
            let targets = self.debug_targets().into_iter().filter_map(|(name, id)| {
                let (x, y) = self.debug_exposed_point(context, id)?;
                let bounds = context.world().layout_box(id)?;
                let (left, top) = context.world().layout_pointer_position(id, bounds.x, bounds.y)?;
                Some(json!({"id":name,"bounds":{"x":left,"y":top,"width":bounds.width,"height":bounds.height},"exposedPoint":{"x":x,"y":y}}))
            }).collect::<Vec<_>>();
            return Ok(Some(json!({"windowId":self.window_id.0,"targets":targets})));
        }
        let target = match command {
            DebugCommand::UiClick { target_id }
            | DebugCommand::UiInput { target_id, .. }
            | DebugCommand::UiKey { target_id, .. } => target_id,
            _ => return Ok(None),
        };
        let Some((_, id)) = self
            .debug_targets()
            .into_iter()
            .find(|(name, _)| *name == target)
        else {
            return Ok(None);
        };
        if self.debug_exposed_point(context, id).is_none() {
            return Ok(None);
        }
        context.focus_node(doc, id)?;
        let accepted = match command {
            DebugCommand::UiClick { .. } => context.activate_node(id)?,
            DebugCommand::UiInput { text, .. } if id == self.composer.stable_id() => {
                context.select_all_focused_text(doc)?;
                context.replace_focused_text(doc, text)?
            }
            DebugCommand::UiKey { key, .. } => {
                nana_ui::RuntimeInputAdapter::default()
                    .dispatch(context, doc, &crate::agent_debug::retained_key_event(key))?
                    .prevent_default
            }
            _ => false,
        };
        Ok(accepted.then(|| json!({"dispatched":true,"windowId":self.window_id.0})))
    }
}
