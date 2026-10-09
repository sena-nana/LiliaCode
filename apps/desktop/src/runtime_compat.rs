use nana_ui::{RuntimeProgramContext, RuntimeProgramUpdate, WindowChromeAction};
use nana_ui_platform::host::WindowCommand;
use nana_ui_platform::{WindowDescriptor, WindowEvent, WindowGeometry, WindowId};

pub type HostedProgramContext<M> = RuntimeProgramContext<M>;
pub type HostedProgramUpdate = RuntimeProgramUpdate;

pub trait HostedUpdateExt {
    fn redraw_primary() -> Self;
    fn redraw_window(id: WindowId) -> Self;
    fn with_window_commands(self, commands: impl IntoIterator<Item = WindowCommand>) -> Self;
}

impl HostedUpdateExt for RuntimeProgramUpdate {
    fn redraw_primary() -> Self {
        Self::redraw(WindowId::PRIMARY)
    }

    fn redraw_window(id: WindowId) -> Self {
        Self::redraw(id)
    }

    fn with_window_commands(mut self, commands: impl IntoIterator<Item = WindowCommand>) -> Self {
        self.window_commands.extend(commands);
        self
    }
}

pub type HostedWindowId = WindowId;
pub type HostedWindowEvent = WindowEvent;
pub type HostedWindowCommand = WindowCommand;
pub type HostedWindowGeometry = WindowGeometry;

#[derive(Debug, Clone, PartialEq)]
pub enum HostedUiCommand {
    Focus { window_id: WindowId, target: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostedWindowAction {
    pub id: WindowId,
    pub action: WindowChromeAction,
}

impl HostedWindowAction {
    pub fn into_window_command(self) -> Option<WindowCommand> {
        match self.action {
            WindowChromeAction::Drag => Some(WindowCommand::Drag(self.id)),
            WindowChromeAction::Minimize => Some(WindowCommand::SetMinimized {
                id: self.id,
                minimized: true,
            }),
            WindowChromeAction::Close => Some(WindowCommand::Close(self.id)),
            WindowChromeAction::ToggleMaximize => Some(WindowCommand::SetMaximized {
                id: self.id,
                maximized: true,
            }),
        }
    }
}

pub fn window_event_id(event: &WindowEvent) -> WindowId {
    match event {
        WindowEvent::OpenFailed { id, .. }
        | WindowEvent::MousePassthroughChanged { id, .. }
        | WindowEvent::Ready { id, .. }
        | WindowEvent::Resized { id, .. }
        | WindowEvent::Moved { id, .. }
        | WindowEvent::VisibilityChanged { id, .. }
        | WindowEvent::FocusChanged { id, .. }
        | WindowEvent::CloseRequested { id }
        | WindowEvent::Closed { id }
        | WindowEvent::FileDialogCompleted { id, .. }
        | WindowEvent::FileDialogRejected { id, .. }
        | WindowEvent::AppearanceChanged { id, .. }
        | WindowEvent::SkipTaskbarChanged { id, .. }
        | WindowEvent::ReducedMotionChanged { id, .. }
        | WindowEvent::HighContrastChanged { id, .. }
        | WindowEvent::PointerPresenceChanged { id, .. }
        | WindowEvent::ModeChanged { id, .. } => *id,
    }
}

pub fn tool_window_settings(
    title: impl Into<String>,
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
) -> WindowDescriptor {
    let mut settings = WindowDescriptor::new(title)
        .initial_size(width, height)
        .minimum_size(min_width, min_height);
    settings.role = nana_ui_platform::WindowRole::Tool;
    settings.parent = Some(WindowId::PRIMARY);
    settings
}

pub fn startup_document(
    status: &str,
) -> Result<nana_ui::runtime::RuntimeDocument, nana_ui::runtime::FrameworkError> {
    let document_id = nana_ui::runtime::DocumentId::new(1).expect("startup document id");
    let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
    use nana_ui::runtime::view::widget;
    let status = status.to_owned();
    document
        .context_mut()
        .mount_view_root(document_id, move || {
            widget(nana_ui::runtime::Stack::column(0.0)).children((
                widget(nana_ui::runtime::Text::new("LiliaCode")),
                widget(nana_ui::runtime::Text::new(status)),
            ))
        })?;
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_event_id_reads_the_window_on_every_variant() {
        let id = WindowId(7);
        assert_eq!(
            window_event_id(&WindowEvent::Ready {
                id,
                geometry: WindowGeometry::default(),
            }),
            id
        );
        assert_eq!(window_event_id(&WindowEvent::CloseRequested { id }), id);
        assert_eq!(window_event_id(&WindowEvent::Closed { id }), id);
    }
}
