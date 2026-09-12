use super::{intent, ConversationAction, ConversationControls, Sink};
use crate::runtime_shell::{emit, ShellIntent};
use nana_ui::runtime::{AppContext, Entity, FrameworkError, KeyInput, TextArea};
use nana_ui_platform::WindowId;
use std::sync::{Arc, Mutex};

impl super::ComposerPasteRequest {
    pub(crate) fn prepare_edit(
        &self,
        context: &mut AppContext,
        document: nana_ui::runtime::DocumentId,
        node: nana_ui::runtime::StableNodeId,
        text: &str,
    ) -> Result<Option<nana_ui::runtime::TextInputState>, FrameworkError> {
        if context.world().text_input(node) != Some(&self.editor)
            || !context
                .world()
                .accessibility(node)
                .is_some_and(|state| state.editable && !state.disabled)
            || !context.world().is_mounted(node)
            || context.world().ime(node).is_some()
        {
            return Ok(None);
        }
        if context.world().focused(document) != Some(node) && !context.focus_node(document, node)? {
            return Ok(None);
        }
        let mut next = context.read(Entity::<TextArea>::from_stable_id(node), Clone::clone)?;
        let applied = next.replace_selection(text);
        Ok((applied && next.state != self.editor).then_some(next.state))
    }
}

#[derive(Default)]
struct State {
    content: String,
    paste_binding: Option<(String, u64, Option<String>)>,
    candidates: Vec<ConversationAction>,
    active: usize,
    can_send: bool,
    locked: bool,
    submitted: bool,
}

#[derive(Default)]
pub(super) struct ComposerKeyboard(Arc<Mutex<State>>);

impl ComposerKeyboard {
    pub fn bind(
        &self,
        context: &mut AppContext,
        composer: Entity<TextArea>,
        window: WindowId,
        sink: Sink,
    ) -> Result<(), FrameworkError> {
        let state = self.0.clone();
        context.on_view_key(composer, move |editor, key: &KeyInput| {
            if key.pressed
                && (key.modifiers.control || key.modifiers.meta)
                && !key.modifiers.alt
                && !key.modifiers.shift
                && key.key.eq_ignore_ascii_case("v")
            {
                let state = state.lock().unwrap_or_else(|error| error.into_inner());
                if !key.repeat && !state.locked {
                    if let Some(binding) = &state.paste_binding {
                        emit(
                            &sink,
                            intent(
                                window,
                                ConversationAction::PasteClipboard(super::ComposerPasteRequest {
                                    binding: binding.clone(),
                                    editor: editor.state.clone(),
                                }),
                            ),
                        );
                    }
                }
                return true;
            }
            if !key.pressed
                || key.modifiers.shift
                || key.modifiers.control
                || key.modifiers.meta
                || key.modifiers.alt
            {
                return false;
            }
            let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
            let action = match key.key.as_ref() {
                "ArrowDown" | "ArrowUp" if !state.candidates.is_empty() && !state.locked => {
                    let count = state.candidates.len();
                    state.active = if key.key.as_ref() == "ArrowDown" {
                        (state.active + 1) % count
                    } else {
                        (state.active + count - 1) % count
                    };
                    Some(intent(window, ConversationAction::CompletionChanged))
                }
                "Enter" | "Tab" if !state.candidates.is_empty() => {
                    if key.repeat || state.locked || state.submitted {
                        return true;
                    }
                    state.submitted = true;
                    Some(intent(window, state.candidates[state.active].clone()))
                }
                "Enter" => {
                    if key.repeat || state.submitted || !state.can_send || state.locked {
                        return true;
                    }
                    state.submitted = true;
                    Some(if window == WindowId::PRIMARY {
                        ShellIntent::SubmitTurn
                    } else {
                        ShellIntent::TaskPopupSubmit(window)
                    })
                }
                _ => None,
            };
            drop(state);
            if let Some(action) = action {
                emit(&sink, action);
                true
            } else {
                false
            }
        })
    }

    pub fn sync(&self, controls: &ConversationControls, content: &str, can_send: bool) {
        let candidates = controls
            .reference_results
            .iter()
            .map(|(id, _)| ConversationAction::SelectReference(id.clone()))
            .chain(
                controls
                    .slash_results
                    .iter()
                    .map(|(id, _)| ConversationAction::Slash(id.clone())),
            )
            .chain(
                controls
                    .context_results
                    .iter()
                    .map(|(id, _)| ConversationAction::Context(id.clone())),
            )
            .collect::<Vec<_>>();
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if state.content != content || state.candidates != candidates {
            state.active = 0;
            state.submitted = false;
        }
        state.content = content.into();
        state.paste_binding = controls.paste_binding.clone();
        state.candidates = candidates;
        state.can_send = can_send;
        state.locked = controls.locked;
    }

    pub fn is_active(&self, action: &ConversationAction) -> bool {
        let state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        state.candidates.get(state.active) == Some(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nana_ui::{
        runtime::{DocumentId, Stack},
        RuntimeInputAdapter,
    };
    use nana_ui_platform::{InputEvent, InputModifiers};

    fn key(name: &str, shift: bool, repeat: bool) -> InputEvent {
        InputEvent::Keyboard {
            pressed: true,
            key: name.into(),
            code: name.into(),
            text: None,
            repeat,
            modifiers: InputModifiers {
                shift,
                ..Default::default()
            },
        }
    }

    fn fixture(
        window: WindowId,
    ) -> (
        AppContext,
        DocumentId,
        Entity<TextArea>,
        ComposerKeyboard,
        Arc<Mutex<Vec<ShellIntent>>>,
    ) {
        let mut context = AppContext::new();
        let document = DocumentId::new(window.0 + 901).unwrap();
        let root = context
            .create_component(document, Stack::column(0.0))
            .unwrap();
        let editor = context
            .create_detached_component(document, TextArea::new("草稿"))
            .unwrap();
        context.append_child(root, editor).unwrap();
        assert!(context.focus_node(document, editor.stable_id()).unwrap());
        let keyboard = ComposerKeyboard::default();
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        keyboard
            .bind(
                &mut context,
                editor,
                window,
                Arc::new(move |intent| sink.lock().unwrap().push(intent)),
            )
            .unwrap();
        keyboard.sync(&ConversationControls::default(), "草稿", true);
        (context, document, editor, keyboard, events)
    }

    #[test]
    fn rich_paste_preflight_rejects_stale_selection_disabled_and_false_edit_before_commit() {
        let (mut cx, doc, editor, _, _) = fixture(WindowId::PRIMARY);
        let request = super::super::ComposerPasteRequest {
            binding: ("task".into(), 1, None),
            editor: cx.read(editor, |view| view.state.clone()).unwrap(),
        };
        let next = request
            .prepare_edit(&mut cx, doc, editor.stable_id(), "正文")
            .unwrap()
            .unwrap();
        assert!(next.value.contains("正文"));
        cx.update_component(editor, |view, _| view.state.selection.anchor = 0)
            .unwrap();
        assert!(request
            .prepare_edit(&mut cx, doc, editor.stable_id(), "引用")
            .unwrap()
            .is_none());
        cx.update_component(editor, |view, _| {
            view.state = request.editor.clone();
            view.disabled = true;
        })
        .unwrap();
        assert!(request
            .prepare_edit(&mut cx, doc, editor.stable_id(), "引用")
            .unwrap()
            .is_none());
        cx.update_component(editor, |view, _| {
            view.disabled = false;
            view.read_only = true;
        })
        .unwrap();
        assert!(request
            .prepare_edit(&mut cx, doc, editor.stable_id(), "引用")
            .unwrap()
            .is_none());
        cx.update_component(editor, |view, _| view.read_only = false)
            .unwrap();
        assert!(request
            .prepare_edit(&mut cx, doc, editor.stable_id(), "")
            .unwrap()
            .is_none());
        cx.remove_view(editor).unwrap();
        assert!(request
            .prepare_edit(&mut cx, doc, editor.stable_id(), "引用")
            .unwrap()
            .is_none());
    }

    #[test]
    fn rich_paste_key_captures_current_selection_once_and_defers_default_paste() {
        for (window, task, revision) in [
            (WindowId::PRIMARY, "task", 7),
            (WindowId(44), "task", 7),
            (WindowId::PRIMARY, "", 0),
        ] {
            let (mut cx, doc, editor, keyboard, events) = fixture(window);
            keyboard.sync(
                &ConversationControls {
                    paste_binding: Some((task.into(), revision, None)),
                    ..Default::default()
                },
                "草稿",
                true,
            );
            cx.update_component(editor, |field, _| {
                field.state.selection = nana_ui::runtime::TextSelection {
                    anchor: 0,
                    focus: "草".len(),
                };
            })
            .unwrap();
            let mut adapter = RuntimeInputAdapter::default().with_clipboard(
                nana_ui_platform::shared_clipboard(nana_ui_platform::MemoryClipboard::new()),
            );
            let paste = InputEvent::Keyboard {
                pressed: true,
                key: "v".into(),
                code: "KeyV".into(),
                text: Some("v".into()),
                repeat: false,
                modifiers: InputModifiers {
                    meta: true,
                    ..Default::default()
                },
            };
            adapter.dispatch(&mut cx, doc, &paste).unwrap();
            assert_eq!(
                cx.read(editor, |view| view.state.value.clone()).unwrap(),
                "草稿"
            );
            let requests = events.lock().unwrap();
            assert_eq!(requests.len(), 1);
            assert!(
                matches!(&requests[0], ShellIntent::Conversation { window_id, action: ConversationAction::PasteClipboard(request) } if *window_id == window && request.binding.1 == revision && request.binding.0 == task && request.editor.selection.focus == "草".len())
            );
            drop(requests);
            cx.set_ime_preedit(doc, "候选".into(), None).unwrap();
            adapter.dispatch(&mut cx, doc, &paste).unwrap();
            assert_eq!(events.lock().unwrap().len(), 1);
        }
    }

    #[test]
    fn enter_sends_once_and_shift_enter_edits_in_both_windows() {
        for window in [WindowId::PRIMARY, WindowId(42)] {
            let (mut context, document, editor, keyboard, events) = fixture(window);
            let mut input = RuntimeInputAdapter::default();
            for repeat in [false, true, false] {
                assert!(
                    input
                        .dispatch(&mut context, document, &key("Enter", false, repeat))
                        .unwrap()
                        .prevent_default
                );
            }
            let events = events.lock().unwrap();
            assert_eq!(events.len(), 1);
            assert!(
                matches!(&events[0], ShellIntent::SubmitTurn) && window == WindowId::PRIMARY
                    || matches!(&events[0], ShellIntent::TaskPopupSubmit(id) if *id == window)
            );
            drop(events);
            assert_eq!(
                context
                    .world()
                    .text_input(editor.stable_id())
                    .unwrap()
                    .value,
                "草稿"
            );
            assert!(
                input
                    .dispatch(&mut context, document, &key("Enter", true, false))
                    .unwrap()
                    .prevent_default
            );
            let value = &context
                .world()
                .text_input(editor.stable_id())
                .unwrap()
                .value;
            assert_eq!(value, "草稿\n");
            keyboard.sync(&ConversationControls::default(), value, true);
        }
    }

    #[test]
    fn candidates_use_arrows_and_enter_or_tab_without_submitting_or_editing() {
        for window in [WindowId::PRIMARY, WindowId(42)] {
            for kind in ["reference", "slash", "context"] {
                let (mut context, document, editor, keyboard, events) = fixture(window);
                let mut controls = ConversationControls::default();
                let entries = vec![
                    ("first".into(), "第一项".into()),
                    ("second".into(), "第二项".into()),
                ];
                let selected = match kind {
                    "reference" => {
                        controls.reference_results = entries;
                        ConversationAction::SelectReference("second".into())
                    }
                    "slash" => {
                        controls.slash_results = entries;
                        ConversationAction::Slash("second".into())
                    }
                    _ => {
                        controls.context_results = entries;
                        ConversationAction::Context("second".into())
                    }
                };
                keyboard.sync(&controls, "草稿", true);
                let mut input = RuntimeInputAdapter::default();
                for name in ["ArrowDown", "ArrowUp", "ArrowUp"] {
                    assert!(
                        input
                            .dispatch(&mut context, document, &key(name, false, false))
                            .unwrap()
                            .prevent_default
                    );
                }
                assert!(keyboard.is_active(&selected));
                let accept = if kind == "slash" { "Tab" } else { "Enter" };
                input
                    .dispatch(&mut context, document, &key(accept, false, false))
                    .unwrap();
                input
                    .dispatch(&mut context, document, &key(accept, false, true))
                    .unwrap();
                let actions = events.lock().unwrap();
                assert_eq!(actions.len(), 4);
                assert!(
                    matches!(&actions[3], ShellIntent::Conversation { window_id, action } if *window_id == window && *action == selected)
                );
                assert_eq!(
                    context
                        .world()
                        .text_input(editor.stable_id())
                        .unwrap()
                        .value,
                    "草稿"
                );
            }
        }
    }

    #[test]
    fn disabled_composer_other_focus_and_ime_never_submit() {
        let (mut context, document, editor, keyboard, events) = fixture(WindowId(42));
        let mut input = RuntimeInputAdapter::default();
        keyboard.sync(&ConversationControls::default(), "草稿", false);
        input
            .dispatch(&mut context, document, &key("Enter", false, false))
            .unwrap();
        assert!(events.lock().unwrap().is_empty());
        keyboard.sync(&ConversationControls::default(), "草稿", true);
        context
            .set_ime_preedit(document, "拼音".into(), None)
            .unwrap();
        input
            .dispatch(&mut context, document, &key("Enter", false, false))
            .unwrap();
        assert!(events.lock().unwrap().is_empty());
        context.clear_ime(document).unwrap();
        let other = context
            .create_component(document, TextArea::new("其他字段"))
            .unwrap();
        context.focus_node(document, other.stable_id()).unwrap();
        input
            .dispatch(&mut context, document, &key("Enter", false, false))
            .unwrap();
        assert!(events.lock().unwrap().is_empty());
        assert_eq!(
            context.world().text_input(other.stable_id()).unwrap().value,
            "其他字段\n"
        );
        context.remove_view(editor).unwrap();
        input
            .dispatch(&mut context, document, &key("Enter", false, false))
            .unwrap();
        assert!(events.lock().unwrap().is_empty());
    }
}
