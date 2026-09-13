use crate::runtime_compat::HostedWindowId;
use crate::runtime_layout::reconcile_children;
use crate::runtime_shell::{
    ShellIntent, ShellPaneLayout, ShellPaneRow, ShellPaneTarget, WorkspacePaneView,
};
use nana_ui::runtime::{
    AppContext, DocumentId, Entity, FrameworkError, MutationQueue, SplitPane, StableNodeId, Stack,
};
use nana_ui::{SplitAxis, SplitPaneModel, SplitPaneMutation};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum CompactSurface {
    #[default]
    Conversation,
    Resources,
}

#[derive(Clone)]
pub(crate) struct CompactWorkbench {
    pub(crate) root: Entity<Stack>,
    header: Entity<Stack>,
    body: Entity<Stack>,
    resources: Entity<Stack>,
    conversation: StableNodeId,
    choice: Entity<nana_ui::runtime::Button>,
    selected: Arc<Mutex<CompactSurface>>,
}

impl CompactWorkbench {
    pub(crate) fn reveal_resources(&self) {
        *self.selected.lock().unwrap() = CompactSurface::Resources;
    }

    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        conversation: StableNodeId,
    ) -> Result<Self, FrameworkError> {
        use nana_ui::runtime::{Activate, Button};
        let root = context.create_detached_component(document, Stack::fill_column(0.0))?;
        let header = context.create_detached_component(document, Stack::row(4.0).padding(8.0))?;
        let body = context.create_detached_component(document, Stack::fill_column(0.0))?;
        let resources = context.create_detached_component(document, Stack::fill_column(0.0))?;
        let choice = context.create_detached_component(document, Button::new("切换到工作区"))?;
        let view = Self {
            root,
            header,
            body,
            resources,
            conversation,
            choice,
            selected: Arc::new(Mutex::new(CompactSurface::default())),
        };
        context.world_mut().register_focus_scope(conversation)?;
        context
            .world_mut()
            .register_focus_scope(resources.stable_id())?;
        let selected = Arc::clone(&view.selected);
        context.on(choice, move |button, _: &Activate, cx| {
            let mut selected = selected.lock().unwrap();
            *selected = match *selected {
                CompactSurface::Conversation => CompactSurface::Resources,
                CompactSurface::Resources => CompactSurface::Conversation,
            };
            let (show, hide) = match *selected {
                CompactSurface::Conversation => (conversation, resources.stable_id()),
                CompactSurface::Resources => (resources.stable_id(), conversation),
            };
            button.label = match *selected {
                CompactSurface::Conversation => "切换到工作区",
                CompactSurface::Resources => "返回对话",
            }
            .to_owned();
            cx.mutations().park_subtree(hide);
            cx.mutations().insert(body.stable_id(), show, None);
            cx.mutations().restore_focus_within(show);
        })?;
        context.append_child(header, choice)?;
        context.append_child(root, header)?;
        context.append_child(root, body)?;
        Ok(view)
    }

    pub(crate) fn sync(
        &self,
        context: &mut AppContext,
        resources: &[StableNodeId],
    ) -> Result<(), FrameworkError> {
        let restore_conversation = resources.is_empty()
            && context
                .world()
                .node(self.choice.stable_id())
                .is_some_and(|node| {
                    context.world().focused(node.document) == Some(self.choice.stable_id())
                });
        let children = if resources.is_empty() {
            vec![self.body.stable_id()]
        } else {
            vec![self.header.stable_id(), self.body.stable_id()]
        };
        reconcile_children(context, self.root.stable_id(), &children)?;
        reconcile_children(context, self.resources.stable_id(), resources)?;
        context
            .world_mut()
            .register_focus_scope(self.conversation)?;
        context
            .world_mut()
            .register_focus_scope(self.resources.stable_id())?;
        context.update_component(self.choice, |button, _| {
            button.disabled = resources.is_empty();
        })?;
        if resources.is_empty() {
            *self.selected.lock().unwrap() = CompactSurface::Conversation;
        }
        self.present(context)?;
        if restore_conversation {
            let mut changes = MutationQueue::new();
            changes.restore_focus_within(self.conversation);
            context.commit_mutations(changes)?;
        }
        Ok(())
    }

    fn present(&self, context: &mut AppContext) -> Result<(), FrameworkError> {
        let selected = *self.selected.lock().unwrap();
        context.update_component(self.choice, |button, _| {
            button.kind = nana_ui::ButtonKind::Subtle;
            button.label = match selected {
                CompactSurface::Conversation => "切换到工作区",
                CompactSurface::Resources => "返回对话",
            }
            .to_owned();
        })?;
        let target = match selected {
            CompactSurface::Conversation => self.conversation,
            CompactSurface::Resources => self.resources.stable_id(),
        };
        let changed = context
            .world()
            .node(self.body.stable_id())
            .is_none_or(|node| node.children.as_slice() != [target]);
        reconcile_children(context, self.body.stable_id(), &[target])?;
        if changed {
            let mut changes = MutationQueue::new();
            changes.restore_focus_within(target);
            context.commit_mutations(changes)?;
        }
        Ok(())
    }
}

type IntentSink = Arc<dyn Fn(ShellIntent) + Send + Sync>;

struct WorkspaceSplit {
    view: Entity<SplitPane>,
    handle: Entity<Stack>,
    size: f32,
    extent: f32,
}

pub(crate) struct WorkspaceView {
    pub(crate) root: Entity<Stack>,
    main: Entity<Stack>,
    bottom: Entity<Stack>,
    bottom_visible: bool,
    compact: Option<CompactWorkbench>,
    reveal_resources: bool,
    window: HostedWindowId,
    sink: IntentSink,
    panes: HashMap<String, WorkspacePaneView>,
    splits: HashMap<String, WorkspaceSplit>,
}

impl WorkspaceView {
    pub(crate) fn reveal_resources(&mut self) {
        self.reveal_resources = true;
    }

    #[cfg(debug_assertions)]
    pub(crate) fn debug_browser_views(&self) -> Vec<crate::browser_workbench::BrowserView> {
        self.panes
            .values()
            .map(|pane| pane.browser.clone())
            .collect()
    }

    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        window: HostedWindowId,
        sink: IntentSink,
    ) -> Result<Self, FrameworkError> {
        Ok(Self {
            root: context.create_detached_component(document, Stack::fill_column(0.0))?,
            main: context.create_detached_component(document, Stack::fill_column(0.0))?,
            bottom: context.create_detached_component(document, Stack::fill_column(0.0))?,
            bottom_visible: false,
            compact: None,
            reveal_resources: false,
            window,
            sink,
            panes: HashMap::new(),
            splits: HashMap::new(),
        })
    }
    pub(crate) fn matches_document_search(
        &self,
        target: &ShellPaneTarget,
        editor: StableNodeId,
        feedback: StableNodeId,
    ) -> bool {
        target.window_id == self.window
            && self
                .panes
                .get(&target.pane_id)
                .is_some_and(|pane| pane.matches_document_search(target, editor, feedback))
    }
    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        panes: &[ShellPaneRow],
        layout: &ShellPaneLayout,
        conversation: StableNodeId,
        window_inline_size: f32,
    ) -> Result<(), FrameworkError> {
        let bounds = context.world().layout_box(self.root.stable_id());
        let size = bounds
            .map(|bounds| [bounds.width, bounds.height])
            .unwrap_or([800.0, 600.0]);
        let narrow = window_inline_size > 0.0 && window_inline_size < 700.0;
        let keep = panes
            .iter()
            .map(|pane| pane.id.as_str())
            .collect::<HashSet<_>>();
        for pane in panes {
            if !self.panes.contains_key(&pane.id) {
                let view =
                    WorkspacePaneView::mount(context, document, self.window, &pane.id, &self.sink)?;
                self.panes.insert(pane.id.clone(), view);
            }
            let conversation_slot = (!narrow
                && pane.active
                && (pane.items.iter().any(|item| {
                    item.selected && item.kind == crate::application::TASK_WORKSPACE_ITEM_KIND
                }) || (panes.len() == 1 && pane.items.is_empty())))
            .then_some(conversation);
            self.panes.get_mut(&pane.id).unwrap().sync(
                context,
                self.window,
                pane,
                None,
                conversation_slot,
            )?;
        }
        let mut split_keep = HashSet::new();
        let terminal_layout = layout.filtered(&|id| {
            panes
                .iter()
                .any(|pane| pane.id == id && crate::runtime_shell::pane_is_terminal(pane))
        });
        let main_layout = layout.filtered(&|id| {
            panes.iter().any(|pane| {
                pane.id == id
                    && !crate::runtime_shell::pane_is_terminal(pane)
                    && (!narrow
                        || pane.items.iter().any(|item| {
                            item.selected
                                && item.kind != crate::application::TASK_WORKSPACE_ITEM_KIND
                        }))
            })
        });
        let main_root = main_layout
            .as_ref()
            .map(|layout| self.sync_layout(context, document, layout, size, &mut split_keep))
            .transpose()?;
        let terminal_root = terminal_layout
            .as_ref()
            .map(|layout| {
                self.sync_layout(context, document, layout, [size[0], 200.0], &mut split_keep)
            })
            .transpose()?;
        reconcile_children(
            context,
            self.main.stable_id(),
            &main_root.into_iter().collect::<Vec<_>>(),
        )?;
        reconcile_children(
            context,
            self.bottom.stable_id(),
            &terminal_root.into_iter().collect::<Vec<_>>(),
        )?;
        self.bottom_visible = !narrow && terminal_root.is_some();
        let root = if narrow {
            if self.compact.is_none() {
                self.compact = Some(CompactWorkbench::mount(context, document, conversation)?);
            }
            let mut resources = Vec::new();
            if main_root.is_some() {
                resources.push(self.main.stable_id());
            }
            if terminal_root.is_some() {
                resources.push(self.bottom.stable_id());
            }
            let compact = self.compact.as_ref().unwrap();
            if self.reveal_resources {
                compact.reveal_resources();
            }
            compact.sync(context, &resources)?;
            compact.root.stable_id()
        } else if main_root.is_some() {
            self.main.stable_id()
        } else {
            conversation
        };
        self.reveal_resources = false;
        reconcile_children(context, self.root.stable_id(), &[root])?;
        let stale = self
            .panes
            .keys()
            .filter(|id| !keep.contains(id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        for id in stale {
            if let Some(pane) = self.panes.remove(&id) {
                let mut ancestor = Some(conversation);
                while let Some(node) = ancestor {
                    if node == pane.root() {
                        let mut mutations = MutationQueue::new();
                        mutations.park_subtree(conversation);
                        context.commit_mutations(mutations)?;
                        break;
                    }
                    ancestor = context.world().node(node).and_then(|node| node.parent);
                }
                pane.dispose(context)?;
            }
        }
        let stale = self
            .splits
            .keys()
            .filter(|id| !split_keep.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        for id in stale {
            let split = self.splits.remove(&id).unwrap();
            for pane in self.panes.values() {
                let mut ancestor = context
                    .world()
                    .node(pane.root())
                    .and_then(|node| node.parent);
                while let Some(node) = ancestor {
                    if node == split.view.stable_id() {
                        let mut mutations = MutationQueue::new();
                        mutations.park_subtree(pane.root());
                        context.commit_mutations(mutations)?;
                        break;
                    }
                    ancestor = context.world().node(node).and_then(|node| node.parent);
                }
            }
            if context.world().contains(split.view.stable_id()) {
                context.remove_view(split.view)?;
            }
            if context.world().contains(split.handle.stable_id()) {
                context.remove_view(split.handle)?;
            }
        }
        Ok(())
    }

    pub(crate) fn bottom_slot(&self) -> Option<StableNodeId> {
        self.bottom_visible.then(|| self.bottom.stable_id())
    }
    fn sync_layout(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        layout: &ShellPaneLayout,
        size: [f32; 2],
        keep: &mut HashSet<String>,
    ) -> Result<StableNodeId, FrameworkError> {
        match layout {
            ShellPaneLayout::Leaf(id) => self
                .panes
                .get(id)
                .map(WorkspacePaneView::root)
                .ok_or(FrameworkError::InvalidInput),
            ShellPaneLayout::Split {
                horizontal,
                ratio,
                first,
                second,
            } => {
                let key = format!("{}:{}", first.first_leaf(), second.first_leaf());
                keep.insert(key.clone());
                let axis = if *horizontal {
                    SplitAxis::Horizontal
                } else {
                    SplitAxis::Vertical
                };
                let dimension = usize::from(!*horizontal);
                let extent = size[dimension].max(160.0);
                let mut ratio = ratio.clamp(0.15, 0.85);
                if let Some(split) = self.splits.get(&key) {
                    let current = context.read(split.view, |view| view.model.size())?;
                    if (current - split.size).abs() > 0.5 {
                        ratio = (current / split.extent.max(1.0)).clamp(0.15, 0.85);
                        (self.sink)(ShellIntent::WorkspacePane {
                            window_id: self.window,
                            pane_id: first.first_leaf().to_owned(),
                            intent: Box::new(ShellIntent::ResizeWorkspaceSplit {
                                first_pane_id: first.first_leaf().to_owned(),
                                second_pane_id: second.first_leaf().to_owned(),
                                ratio,
                            }),
                        });
                    }
                }
                let mut first_size = size;
                first_size[dimension] = extent * ratio;
                let mut second_size = size;
                second_size[dimension] = (extent - first_size[dimension] - 6.0).max(0.0);
                let a = self.sync_layout(context, document, first, first_size, keep)?;
                let b = self.sync_layout(context, document, second, second_size, keep)?;
                let desired = extent * ratio;
                if !self.splits.contains_key(&key) {
                    let handle = context.create_detached_component(document, Stack::bar(0.0))?;
                    let view = context.create_detached_component(
                        document,
                        SplitPane::from_model(
                            &SplitPaneModel::new(axis, desired, 0.0, 10_000.0),
                            a,
                            b,
                        )
                        .handle(handle.stable_id()),
                    )?;
                    self.splits.insert(
                        key.clone(),
                        WorkspaceSplit {
                            view,
                            handle,
                            size: desired,
                            extent,
                        },
                    );
                }
                let split = self.splits.get_mut(&key).unwrap();
                context.update_component(split.view, |view, _| {
                    view.first = Some(a);
                    view.second = Some(b);
                    view.handle = Some(split.handle.stable_id());
                    view.model.update(SplitPaneMutation::SetSize(desired));
                })?;
                split.size = desired;
                split.extent = extent;
                reconcile_children(
                    context,
                    split.view.stable_id(),
                    &[a, split.handle.stable_id(), b],
                )?;
                Ok(split.view.stable_id())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_shell::{ShellDocumentSnapshot, ShellPaneItem};
    use nana_ui::runtime::{TextArea, TextChanged};
    use std::sync::Mutex;

    #[test]
    fn compact_switch_restores_focus_without_losing_drafts_or_selection() {
        let mut context = AppContext::new();
        let document = DocumentId::new(406).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let conversation = context
            .create_detached_component(document, TextArea::new("draft"))
            .unwrap();
        let editor = context
            .create_detached_component(document, TextArea::new("unsaved document"))
            .unwrap();
        context
            .update_component(editor, |editor, _| {
                editor.state.selection = nana_ui::runtime::TextSelection {
                    anchor: 2,
                    focus: 8,
                };
            })
            .unwrap();
        let compact =
            CompactWorkbench::mount(&mut context, document, conversation.stable_id()).unwrap();
        context.append_child(host, compact.root).unwrap();
        compact.sync(&mut context, &[editor.stable_id()]).unwrap();
        compact.reveal_resources();
        compact.sync(&mut context, &[editor.stable_id()]).unwrap();
        assert_eq!(
            context
                .world()
                .node(compact.body.stable_id())
                .unwrap()
                .children,
            vec![compact.resources.stable_id()]
        );
        context
            .update_component(compact.choice, |_, cx| cx.emit(nana_ui::runtime::Activate))
            .unwrap();
        compact.sync(&mut context, &[editor.stable_id()]).unwrap();
        assert_eq!(
            context
                .world()
                .node(compact.body.stable_id())
                .unwrap()
                .children,
            vec![conversation.stable_id()]
        );

        context
            .focus_node(document, conversation.stable_id())
            .unwrap();
        context
            .focus_node(document, compact.choice.stable_id())
            .unwrap();
        assert_eq!(
            context
                .world()
                .node(compact.body.stable_id())
                .unwrap()
                .children,
            vec![conversation.stable_id()]
        );
        context
            .update_component(compact.choice, |_, cx| cx.emit(nana_ui::runtime::Activate))
            .unwrap();
        assert_eq!(
            context
                .world()
                .node(compact.body.stable_id())
                .unwrap()
                .children,
            vec![compact.resources.stable_id()]
        );
        context.focus_node(document, editor.stable_id()).unwrap();
        compact.sync(&mut context, &[editor.stable_id()]).unwrap();
        assert_eq!(
            context
                .world()
                .node(compact.body.stable_id())
                .unwrap()
                .children,
            vec![compact.resources.stable_id()]
        );
        context
            .focus_node(document, compact.choice.stable_id())
            .unwrap();
        context
            .update_component(compact.choice, |_, cx| cx.emit(nana_ui::runtime::Activate))
            .unwrap();
        assert_eq!(
            context.world().focused(document),
            Some(conversation.stable_id())
        );
        assert_eq!(
            context
                .read(conversation, |editor| editor.state.value.clone())
                .unwrap(),
            "draft"
        );
        assert_eq!(
            context
                .read(editor, |editor| (
                    editor.state.value.clone(),
                    editor.state.selection
                ))
                .unwrap(),
            (
                "unsaved document".to_owned(),
                nana_ui::runtime::TextSelection {
                    anchor: 2,
                    focus: 8
                }
            )
        );
        context
            .focus_node(document, compact.choice.stable_id())
            .unwrap();
        context
            .update_component(compact.choice, |_, cx| cx.emit(nana_ui::runtime::Activate))
            .unwrap();
        assert_eq!(context.world().focused(document), Some(editor.stable_id()));
        compact.sync(&mut context, &[]).unwrap();
        assert_eq!(
            context.world().focused(document),
            Some(conversation.stable_id())
        );
        assert!(
            context
                .read(compact.choice, |button| button.disabled)
                .unwrap()
        );
        assert!(context.world().contains(editor.stable_id()));
        assert_eq!(
            context
                .world()
                .node(compact.root.stable_id())
                .unwrap()
                .children,
            vec![compact.body.stable_id()]
        );
        compact.sync(&mut context, &[editor.stable_id()]).unwrap();
        assert_eq!(
            context
                .world()
                .node(compact.root.stable_id())
                .unwrap()
                .children,
            vec![compact.header.stable_id(), compact.body.stable_id()]
        );
        context
            .focus_node(document, compact.choice.stable_id())
            .unwrap();
        compact.sync(&mut context, &[]).unwrap();
        assert_eq!(
            context.world().focused(document),
            Some(conversation.stable_id())
        );
    }

    fn document_pane(pane: &str, item: &str, text: &str) -> ShellPaneRow {
        ShellPaneRow {
            id: pane.into(),
            active: true,
            items: vec![ShellPaneItem {
                id: item.into(),
                title: item.into(),
                kind: crate::application::DOCUMENT_WORKSPACE_ITEM_KIND.into(),
                selected: true,
                closable: true,
            }],
            document: Some(ShellDocumentSnapshot {
                item_id: item.into(),
                revision: 7,
                conflicted: false,
                title: item.into(),
                text: text.into(),
                language: "rust".into(),
                status: String::new(),
                read_only: false,
                dirty: false,
                diagnostics: Vec::new(),
            }),
            terminal: None,
            browser: None,
        }
    }
    #[test]
    fn popup_panes_emit_window_scoped_edits_and_preserve_the_other_view() {
        let mut context = AppContext::new();
        let document = DocumentId::new(404).unwrap();
        let window = nana_ui_platform::WindowId(27);
        let intents = Arc::new(Mutex::new(Vec::new()));
        let observed = intents.clone();
        let sink: IntentSink = Arc::new(move |intent| observed.lock().unwrap().push(intent));
        let mut view = WorkspaceView::mount(&mut context, document, window, sink).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        context.append_child(host, view.root).unwrap();
        let conversation = context
            .create_detached_component(document, TextArea::new("conversation"))
            .unwrap();
        let panes = [
            document_pane("left", "a", "alpha"),
            document_pane("right", "b", "beta"),
        ];
        let layout = ShellPaneLayout::Split {
            horizontal: true,
            ratio: 0.5,
            first: Box::new(ShellPaneLayout::Leaf("left".into())),
            second: Box::new(ShellPaneLayout::Leaf("right".into())),
        };
        view.sync(
            &mut context,
            document,
            &panes,
            &layout,
            conversation.stable_id(),
            800.0,
        )
        .unwrap();
        let editor = context
            .world()
            .document_order(document)
            .into_iter()
            .find_map(|id| {
                let entity = Entity::<TextArea>::from_stable_id(id);
                context
                    .read(entity, |view| view.state.value == "alpha")
                    .ok()
                    .filter(|found| *found)
                    .map(|_| entity)
            })
            .unwrap();
        context
            .update_component(editor, |editor, cx| {
                editor.state.replace_value("changed".to_owned());
                cx.emit(TextChanged {
                    value: "changed".into(),
                    selection: editor.state.selection,
                });
            })
            .unwrap();
        assert!(intents.lock().unwrap().iter().any(|intent|matches!(intent,ShellIntent::DocumentChanged{target,revision:7,value} if target.window_id==window && target.pane_id=="left" && target.item_id=="a" && value=="changed")));
        assert!(
            context
                .world()
                .document_order(document)
                .into_iter()
                .any(|id| context
                    .read(Entity::<TextArea>::from_stable_id(id), |view| view
                        .state
                        .value
                        == "beta")
                    .unwrap_or(false))
        );
        let old_right = view.panes["right"].root();
        view.sync(
            &mut context,
            document,
            &panes[..1],
            &ShellPaneLayout::Leaf("left".into()),
            conversation.stable_id(),
            800.0,
        )
        .unwrap();
        assert!(!context.world().contains(old_right));
        assert!(context.world().contains(editor.stable_id()));
        view.sync(
            &mut context,
            document,
            &panes[..1],
            &ShellPaneLayout::Leaf("left".into()),
            conversation.stable_id(),
            600.0,
        )
        .unwrap();
        assert_eq!(
            context
                .world()
                .node(view.root.stable_id())
                .unwrap()
                .children,
            vec![view.compact.as_ref().unwrap().root.stable_id()]
        );
        view.sync(
            &mut context,
            document,
            &panes[..1],
            &ShellPaneLayout::Leaf("left".into()),
            conversation.stable_id(),
            900.0,
        )
        .unwrap();
        assert_eq!(
            context
                .world()
                .node(view.root.stable_id())
                .unwrap()
                .children,
            vec![view.main.stable_id()]
        );
        assert!(context.world().contains(editor.stable_id()));
    }
    #[test]
    fn task_pane_sync_keeps_composer_focus_and_closing_keeps_the_reusable_conversation() {
        let mut context = AppContext::new();
        let document = DocumentId::new(405).unwrap();
        let mut view = WorkspaceView::mount(
            &mut context,
            document,
            nana_ui_platform::WindowId(28),
            Arc::new(|_| {}),
        )
        .unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        context.append_child(host, view.root).unwrap();
        let composer = context
            .create_detached_component(document, TextArea::new("draft"))
            .unwrap();
        let mut pane = document_pane("task", "task-item", "");
        pane.items[0].kind = crate::application::TASK_WORKSPACE_ITEM_KIND.into();
        pane.document = None;
        let layout = ShellPaneLayout::Leaf("task".into());
        view.sync(
            &mut context,
            document,
            &[pane.clone()],
            &layout,
            composer.stable_id(),
            800.0,
        )
        .unwrap();
        context.focus_node(document, composer.stable_id()).unwrap();
        view.sync(
            &mut context,
            document,
            &[pane],
            &layout,
            composer.stable_id(),
            800.0,
        )
        .unwrap();
        assert_eq!(
            context.world().focused(document),
            Some(composer.stable_id())
        );
        view.sync(
            &mut context,
            document,
            &[],
            &ShellPaneLayout::default(),
            composer.stable_id(),
            800.0,
        )
        .unwrap();
        assert!(context.world().contains(composer.stable_id()));
        assert_eq!(
            context
                .world()
                .node(view.root.stable_id())
                .unwrap()
                .children,
            vec![composer.stable_id()]
        );
    }
}
