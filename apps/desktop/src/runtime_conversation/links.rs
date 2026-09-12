use super::*;
use nana_ui::runtime::RichTextEvent;

pub(super) fn selection_toolbar(
    context: &AppContext,
    markdown: Entity<NativeMarkdown>,
    parent: Entity<Stack>,
    selected: bool,
) -> Stack {
    let plain = Stack::row(6.0);
    if !selected {
        return plain;
    }
    let Some(nana_ui::runtime::ComponentGeometry::NativeMarkdown { selection, .. }) =
        context.world().component_geometry(markdown.stable_id())
    else {
        return plain;
    };
    let Some(parent) = context.world().layout_box(parent.stable_id()) else {
        return plain;
    };
    let Some(first) = selection.first() else {
        return plain;
    };
    let left = selection.iter().map(|rect| rect.x).fold(first.x, f32::min);
    let right = selection
        .iter()
        .map(|rect| rect.x + rect.width)
        .fold(first.x, f32::max);
    let top = selection.iter().map(|rect| rect.y).fold(first.y, f32::min);
    let bottom = selection
        .iter()
        .map(|rect| rect.y + rect.height)
        .fold(first.y, f32::max);
    let x = ((left + right) / 2.0 - parent.x - 90.0).clamp(0.0, (parent.width - 180.0).max(0.0));
    let y = if top - parent.y >= 42.0 {
        top - parent.y - 40.0
    } else {
        bottom - parent.y + 6.0
    };
    plain
        .padding(4.0)
        .radius(8.0)
        .surface(SemanticColorRole::Surface)
        .outline(SemanticColorRole::BorderSoft, 1.0)
        .with_layout(|layout| {
            layout.position = PositionSpec::Absolute;
            layout.offset_left = Some(LengthSpec::Px(x));
            layout.offset_top = Some(LengthSpec::Px(y));
            layout.z_index = Some(100);
        })
}

pub(super) fn bind_markdown_links(
    context: &mut AppContext,
    markdown: Entity<NativeMarkdown>,
    window: WindowId,
    event_id: String,
    sink: Sink,
) -> Result<(), FrameworkError> {
    let escape_sink = sink.clone();
    let escape_event = event_id.clone();
    context.on_key(markdown, move |key| {
        if key.pressed && key.key.as_ref() == "Escape" {
            escape_sink(intent(
                window,
                ConversationAction::SelectText {
                    event_id: escape_event.clone(),
                    text: None,
                },
            ));
            true
        } else {
            false
        }
    })?;
    context.on(markdown, move |_, event: &RichTextEvent, _| match event {
        RichTextEvent::LinkActivated(uri) => sink(ShellIntent::OpenMarkdownLink(uri.to_string())),
        RichTextEvent::ImageActivated(image) => {
            sink(intent(window, ConversationAction::OpenImage(image.clone())))
        }
        RichTextEvent::SelectionChanged(selection) => sink(intent(
            window,
            ConversationAction::SelectText {
                event_id: event_id.clone(),
                text: selection.as_ref().map(|selection| selection.text.clone()),
            },
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::timeline::{TimelineModule, TimelineModuleMessage};
    use crate::ui_module::{UiModule, UiModuleContext};
    use nana_ui::runtime::LayoutViewport;
    use nana_ui::RuntimeInputAdapter;
    use nana_ui_platform::{InputEvent, InputModifiers, PointerPhase, PointerType};
    use std::sync::Mutex;

    fn pointer(phase: PointerPhase, x: f32, y: f32) -> InputEvent {
        InputEvent::Pointer {
            phase,
            pointer_id: 1,
            pointer_type: PointerType::Mouse,
            x,
            y,
            screen_x: x,
            screen_y: y,
            button: 0,
            buttons: u16::from(phase != PointerPhase::Up),
            pressure: 1.0,
            tangential_pressure: 0.0,
            tilt_x: 0,
            tilt_y: 0,
            twist: 0,
            is_primary: true,
            activation_click: false,
            modifiers: InputModifiers::default(),
        }
    }

    #[test]
    fn timeline_pointer_routes_links_images_and_selection_actions_in_each_window() {
        for window in [WindowId::PRIMARY, WindowId(44)] {
            let mut context = AppContext::new();
            let document = DocumentId::new(1).unwrap();
            let root = context
                .create_component(document, Stack::column(0.0))
                .unwrap();
            let events = Arc::new(Mutex::new(Vec::new()));
            let received = events.clone();
            let sink: Sink = Arc::new(move |intent| received.lock().unwrap().push(intent));
            let mut content = TimelineContent::mount(&mut context, document).unwrap();
            let mut row = crate::runtime_shell::ShellTimelineRow {
                id: "reply".into(),
                title: String::new(),
                role: "assistant".into(),
                status: "completed".into(),
                markdown: "[link](https://example.com/a) ![image](https://example.com/a) plain"
                    .into(),
                selected_text: None,
                images: Vec::new(),
                attachments: Vec::new(),
                expanded: true,
                can_expand: false,
                can_retry: false,
                can_copy: true,
                can_branch: false,
                can_apply: false,
            };
            content
                .sync(&mut context, document, root, window, &row, &sink)
                .unwrap();
            context
                .layout_document(document, LayoutViewport::new(600.0, 500.0))
                .unwrap();
            context.rebuild_hit_test(document);
            let bounds = context
                .world()
                .layout_box(content.markdown.stable_id())
                .unwrap();
            let geometry = context
                .read(content.markdown, |view| view.layout(bounds))
                .unwrap();
            let point = |index: usize| {
                let bounds = geometry.blocks[0].graphemes[index].bounds;
                (bounds.x + 1.0, bounds.y + bounds.height / 2.0)
            };
            let mut input = RuntimeInputAdapter::default();
            for index in [0, 5] {
                let (x, y) = point(index);
                for phase in [PointerPhase::Down, PointerPhase::Up] {
                    input
                        .dispatch(&mut context, document, &pointer(phase, x, y))
                        .unwrap();
                }
            }
            {
                let mut events = events.lock().unwrap();
                assert!(
                    matches!(&events[0], ShellIntent::OpenMarkdownLink(uri) if uri == "https://example.com/a")
                );
                assert!(
                    matches!(&events[1], ShellIntent::Conversation { window_id, action: ConversationAction::OpenImage(image) } if *window_id == window && image.source == "https://example.com/a")
                );
                events.clear();
            }
            let (x, y) = point(0);
            let (end_x, end_y) = point(3);
            for event in [
                pointer(PointerPhase::Down, x, y),
                pointer(PointerPhase::Move, end_x, end_y),
                pointer(PointerPhase::Up, end_x, end_y),
            ] {
                input.dispatch(&mut context, document, &event).unwrap();
            }
            let kernel = lilia_kernel::Kernel::new();
            let cx = UiModuleContext::new(&kernel, window);
            let mut module = TimelineModule::default();
            for event in events.lock().unwrap().drain(..) {
                let ShellIntent::Conversation {
                    window_id,
                    action: ConversationAction::SelectText { event_id, text },
                } = event
                else {
                    panic!("selection must not open a link");
                };
                assert_eq!(window_id, window);
                module.reduce(TimelineModuleMessage::SelectText { event_id, text }, &cx);
            }
            assert_eq!(module.text_selection().unwrap().event_id, "reply");
            assert_eq!(module.text_selection().unwrap().text, "lin");
            row.selected_text = module
                .text_selection()
                .map(|selection| selection.text.clone());
            content
                .sync(&mut context, document, root, window, &row, &sink)
                .unwrap();
            context
                .layout_document(document, LayoutViewport::new(600.0, 500.0))
                .unwrap();
            context.rebuild_hit_test(document);
            for id in ["selection-copy", "selection-quote", "selection-ask"] {
                let button = content.buttons[id];
                assert!(context
                    .world()
                    .node(content.actions.stable_id())
                    .unwrap()
                    .children
                    .contains(&button.stable_id()));
                let rect = context.world().layout_box(button.stable_id()).unwrap();
                let (x, y) = context
                    .world()
                    .layout_pointer_position(
                        button.stable_id(),
                        rect.x + rect.width / 2.0,
                        rect.y + rect.height / 2.0,
                    )
                    .unwrap();
                for phase in [PointerPhase::Down, PointerPhase::Up] {
                    input
                        .dispatch(&mut context, document, &pointer(phase, x, y))
                        .unwrap();
                }
            }
            let emitted = events.lock().unwrap();
            assert!(
                matches!(&emitted[0], ShellIntent::Conversation { window_id, action: ConversationAction::CopySelection } if *window_id == window)
            );
            assert!(
                matches!(&emitted[1], ShellIntent::Conversation { window_id, action: ConversationAction::QuoteSelection } if *window_id == window)
            );
            assert!(
                matches!(&emitted[2], ShellIntent::Conversation { window_id, action: ConversationAction::AskSelection } if *window_id == window)
            );
            drop(emitted);
            module.reduce(TimelineModuleMessage::ClearTextSelection, &cx);
            row.selected_text = None;
            content
                .sync(&mut context, document, root, window, &row, &sink)
                .unwrap();
            assert_eq!(
                context
                    .read(content.markdown, |view| view.selected_text())
                    .unwrap(),
                None
            );
            let children = context
                .world()
                .node(content.actions.stable_id())
                .unwrap()
                .children;
            assert!(!children.contains(&content.buttons["selection-copy"].stable_id()));
        }
    }
}
