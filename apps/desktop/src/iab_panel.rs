use std::sync::Arc;

use lilia_contracts::TaskId;
use nana_ui::runtime::{
    Activate, AppContext, BrowserView, Button, DocumentId, Entity, FrameworkError, LengthSpec,
    Stack, Text, TextChanged, TextInput,
};
use nana_ui::{
    BrowserCommand, BrowserEvent, BrowserPolicy, NativeBrowserEvent, NativeBrowserRequest,
};

use crate::runtime_compat::HostedWindowId;
use crate::runtime_shell::ShellIntent;

pub const BROWSER_ID: &str = "task-browser";

#[derive(Debug, Clone)]
pub enum IabAction {
    Open,
    AddressChanged(String),
    Navigate,
    Back,
    Forward,
    Reload,
    Stop,
    Capture,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IabPanelSnapshot {
    pub address: String,
    pub status: String,
    pub available: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub loading: bool,
    pub can_capture: bool,
}

impl Default for IabPanelSnapshot {
    fn default() -> Self {
        Self {
            address: String::new(),
            status: String::new(),
            available: cfg!(target_os = "macos"),
            can_go_back: false,
            can_go_forward: false,
            loading: false,
            can_capture: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IabPanelState {
    active_url: String,
    address: String,
    state: nana_ui::BrowserState,
    visible: bool,
    opened: bool,
    revision: u64,
    command: BrowserCommand,
    capture_revision: Option<u64>,
    pub capture_task: Option<TaskId>,
    pub capture_page: Option<(String, String)>,
}

impl Default for IabPanelState {
    fn default() -> Self {
        Self::new("about:blank")
    }
}

impl IabPanelState {
    pub fn new(initial_url: impl Into<String>) -> Self {
        let url = initial_url.into();
        Self {
            active_url: url.clone(),
            address: if url == "about:blank" {
                String::new()
            } else {
                url.clone()
            },
            state: Default::default(),
            visible: false,
            opened: false,
            revision: 1,
            command: BrowserCommand::Navigate(url),
            capture_revision: None,
            capture_task: None,
            capture_page: None,
        }
    }
    pub fn browser_attached(&self) -> bool {
        self.state.attached
    }
    pub fn browser_ready(&self) -> bool {
        self.state.attached && !self.state.loading && self.state.error.is_none()
    }
    pub fn active_url(&self) -> &str {
        &self.active_url
    }
    pub fn error(&self) -> Option<&str> {
        self.state.error.as_deref()
    }
    pub fn set_panel_visible(&mut self, visible: bool, _window_id: HostedWindowId) {
        if !visible {
            self.cancel_capture();
        } else if !self.visible {
            self.state.attached = false;
            self.state.can_go_back = false;
            self.state.can_go_forward = false;
            self.state.loading = false;
        }
        self.visible = visible;
        self.opened |= visible;
    }
    pub fn set_error(&mut self, error: String) {
        self.state.error = Some(error);
    }
    pub fn snapshot(&self, can_capture: bool) -> IabPanelSnapshot {
        IabPanelSnapshot {
            address: self.address.clone(),
            status: self.state.error.clone().unwrap_or_else(|| {
                if self.state.loading {
                    "正在加载…".into()
                } else if self.active_url == "about:blank" {
                    "输入网址开始浏览".into()
                } else {
                    self.state.title.clone()
                }
            }),
            available: cfg!(target_os = "macos"),
            can_go_back: self.state.can_go_back,
            can_go_forward: self.state.can_go_forward,
            loading: self.state.loading,
            can_capture: can_capture
                && self.visible
                && self.browser_ready()
                && self.capture_task.is_none()
                && self.active_url != "about:blank",
        }
    }
    pub fn request(&self, node: nana_ui::runtime::StableNodeId) -> Option<NativeBrowserRequest> {
        self.opened.then(|| NativeBrowserRequest {
            id: BROWSER_ID.into(),
            node,
            policy: BrowserPolicy { allow_web: true },
            restore_url: self.active_url.clone(),
            visible: self.visible,
            revision: self.revision,
            command: Some(self.command.clone()),
        })
    }
    pub fn cancel_capture(&mut self) {
        self.capture_revision = None;
        self.capture_task = None;
        self.capture_page = None;
    }

    pub fn begin_capture(&mut self, task: TaskId) {
        self.capture_task = Some(task);
        self.capture_page = Some((self.active_url.clone(), self.state.title.clone()));
    }

    pub fn apply(&mut self, action: IabAction) {
        let command = match action {
            IabAction::AddressChanged(address) => {
                self.address = address;
                return;
            }
            IabAction::Navigate => match normalize_address(&self.address) {
                Ok(url) => {
                    self.address = url.clone();
                    BrowserCommand::Navigate(url)
                }
                Err(error) => {
                    self.state.error = Some(error);
                    return;
                }
            },
            IabAction::Back if self.state.can_go_back => BrowserCommand::Back,
            IabAction::Forward if self.state.can_go_forward => BrowserCommand::Forward,
            IabAction::Reload => BrowserCommand::Reload,
            IabAction::Stop => BrowserCommand::Stop,
            IabAction::Capture => BrowserCommand::Capture,
            _ => return,
        };
        let Some(revision) = self.revision.checked_add(1) else {
            self.cancel_capture();
            self.state.error = Some("请重新打开浏览面板后再试".into());
            return;
        };
        if matches!(command, BrowserCommand::Capture) {
            if self.capture_task.is_none() {
                return;
            }
            self.capture_revision = Some(revision);
        } else {
            self.cancel_capture();
        }
        self.state.error = None;
        self.revision = revision;
        self.command = command;
    }
    pub fn receive(&mut self, event: NativeBrowserEvent) -> Option<Vec<u8>> {
        if event.id != BROWSER_ID || event.revision != self.revision {
            return None;
        }
        match event.event {
            BrowserEvent::State(state) => {
                if !state.url.is_empty() && state.url != self.active_url {
                    self.cancel_capture();
                    self.active_url = state.url.clone();
                    self.address = if state.url == "about:blank" {
                        String::new()
                    } else {
                        state.url.clone()
                    };
                }
                self.state = state;
            }
            BrowserEvent::Captured(bytes) => {
                if self.capture_revision == Some(event.revision) && self.capture_task.is_some() {
                    self.capture_revision = None;
                    return Some(bytes);
                }
            }
            BrowserEvent::CaptureFailed(error) => {
                if self.capture_revision == Some(event.revision) {
                    self.cancel_capture();
                    self.state.error = Some(error);
                }
            }
        }
        None
    }
}

fn normalize_address(input: &str) -> Result<String, String> {
    let value = input.trim();
    if value.is_empty() {
        return Err("请输入网址".into());
    }
    if value == "about:blank" {
        return Ok(value.into());
    }
    let candidate = if value.contains("://") {
        value.to_owned()
    } else if value.starts_with("localhost")
        || value.starts_with("127.0.0.1")
        || value.starts_with("[::1]")
    {
        format!("http://{value}")
    } else {
        format!("https://{value}")
    };
    let url = url::Url::parse(&candidate).map_err(|_| "网址无效".to_owned())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("请输入 HTTP 或 HTTPS 网址".into());
    }
    Ok(url.to_string())
}

pub(crate) struct IabPanelView {
    pub root: Entity<Stack>,
    pub browser: Entity<BrowserView>,
    pub address: Entity<TextInput>,
    status: Entity<Text>,
    buttons: Vec<(IabAction, Entity<Button>)>,
}

impl IabPanelView {
    pub fn mount(
        context: &mut AppContext,
        document: DocumentId,
        sink: Arc<dyn Fn(ShellIntent) + Send + Sync>,
    ) -> Result<Self, FrameworkError> {
        let root = context.create_detached_component(document, Stack::fill_column(8.0))?;
        let address_row = context.create_detached_component(document, Stack::bar(6.0))?;
        let address = context.create_detached_component(
            document,
            TextInput::new("").placeholder("输入网址").layout(
                Stack::fill_row(0.0)
                    .height(LengthSpec::Px(32.0))
                    .node_style()
                    .layout,
            ),
        )?;
        context.append_child(root, address_row)?;
        context.append_child(address_row, address)?;
        let action_sink = sink.clone();
        context.on(address, move |_, event: &TextChanged, _| {
            action_sink(ShellIntent::Browser(IabAction::AddressChanged(
                event.value.clone(),
            )))
        })?;
        let toolbar = context.create_detached_component(document, Stack::bar(6.0))?;
        context.append_child(root, toolbar)?;
        let mut buttons = Vec::new();
        for (label, action) in [
            ("打开", IabAction::Navigate),
            ("后退", IabAction::Back),
            ("前进", IabAction::Forward),
            ("刷新", IabAction::Reload),
            ("停止", IabAction::Stop),
            ("截图", IabAction::Capture),
        ] {
            let button = context.create_detached_component(
                document,
                Button::new(label).layout(
                    Stack::row(0.0)
                        .width(LengthSpec::Px(44.0))
                        .height(LengthSpec::Px(32.0))
                        .node_style()
                        .layout,
                ),
            )?;
            context.append_child(
                if matches!(action, IabAction::Navigate) {
                    address_row
                } else {
                    toolbar
                },
                button,
            )?;
            let action_sink = sink.clone();
            let captured = action.clone();
            context.on(button, move |_, _: &Activate, _| {
                action_sink(ShellIntent::Browser(captured.clone()))
            })?;
            buttons.push((action, button));
        }
        let status = context.create_detached_component(document, Text::new("输入网址开始浏览"))?;
        context.append_child(root, status)?;
        let browser = context.create_detached_component(document, BrowserView::new(BROWSER_ID))?;
        context.append_child(root, browser)?;
        Ok(Self {
            root,
            browser,
            address,
            status,
            buttons,
        })
    }
    pub(crate) fn debug_targets(&self) -> Vec<(String, nana_ui::runtime::StableNodeId)> {
        let mut result = vec![
            ("address".into(), self.address.stable_id()),
            ("view".into(), self.browser.stable_id()),
        ];
        for (action, button) in &self.buttons {
            let key = match action {
                IabAction::Navigate => "navigate",
                IabAction::Back => "back",
                IabAction::Forward => "forward",
                IabAction::Reload => "reload",
                IabAction::Stop => "stop",
                IabAction::Capture => "capture",
                _ => continue,
            };
            result.push((key.into(), button.stable_id()));
        }
        result
    }

    pub fn sync(
        &self,
        context: &mut AppContext,
        state: &IabPanelSnapshot,
    ) -> Result<(), FrameworkError> {
        context.update_component(self.address, |input, _| {
            if input.state.value != state.address {
                input.state.replace_value(state.address.clone());
            }
            input.disabled = !state.available;
        })?;
        context.update_component(self.status, |text, _| {
            text.value = if state.available {
                state.status.clone()
            } else {
                "此平台暂不支持内嵌浏览".into()
            }
        })?;
        for (action, button) in &self.buttons {
            context.update_component(*button, |button, _| {
                button.disabled = !state.available
                    || match action {
                        IabAction::Back => !state.can_go_back,
                        IabAction::Forward => !state.can_go_forward,
                        IabAction::Stop => !state.loading,
                        IabAction::Capture => !state.can_capture,
                        _ => false,
                    }
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn completion(revision: u64, event: BrowserEvent) -> NativeBrowserEvent {
        NativeBrowserEvent {
            id: BROWSER_ID.into(),
            node: nana_ui::runtime::StableNodeId::new(1).unwrap(),
            revision,
            event,
        }
    }
    fn start_capture(state: &mut IabPanelState, task: &str) -> u64 {
        state.begin_capture(TaskId::new(task).unwrap());
        state.apply(IabAction::Capture);
        state.revision
    }
    #[test]
    fn delayed_or_duplicate_capture_cannot_attach_to_a_later_task() {
        let mut state = IabPanelState::default();
        let old = start_capture(&mut state, "one");
        state.cancel_capture();
        let current = start_capture(&mut state, "two");
        assert!(state
            .receive(completion(old, BrowserEvent::Captured(vec![1])))
            .is_none());
        assert_eq!(state.capture_task, Some(TaskId::new("two").unwrap()));
        assert_eq!(
            state.receive(completion(current, BrowserEvent::Captured(vec![2]))),
            Some(vec![2])
        );
        assert!(state
            .receive(completion(current, BrowserEvent::Captured(vec![3])))
            .is_none());
    }
    #[test]
    fn navigation_or_hiding_invalidates_an_inflight_capture() {
        let mut state = IabPanelState::default();
        let first = start_capture(&mut state, "one");
        state.apply(IabAction::AddressChanged("example.com".into()));
        state.apply(IabAction::Navigate);
        assert!(state.capture_task.is_none());
        assert!(state
            .receive(completion(first, BrowserEvent::Captured(vec![1])))
            .is_none());
        let second = start_capture(&mut state, "one");
        state.set_panel_visible(false, HostedWindowId::PRIMARY);
        assert!(state
            .receive(completion(second, BrowserEvent::Captured(vec![2])))
            .is_none());
        assert!(state.capture_page.is_none());
    }

    #[test]
    fn reopening_preserves_the_page_address_and_waits_for_fresh_native_state() {
        let mut state = IabPanelState::new("https://example.com/current");
        state.set_panel_visible(true, HostedWindowId::PRIMARY);
        state.receive(completion(
            state.revision,
            BrowserEvent::State(nana_ui::BrowserState {
                attached: true,
                can_go_back: true,
                url: state.active_url.clone(),
                ..Default::default()
            }),
        ));
        start_capture(&mut state, "one");
        state.set_panel_visible(false, HostedWindowId::PRIMARY);
        state.set_panel_visible(true, HostedWindowId::PRIMARY);
        let request = state
            .request(nana_ui::runtime::StableNodeId::new(1).unwrap())
            .unwrap();
        assert_eq!(request.restore_url, "https://example.com/current");
        assert!(!state.browser_attached());
        assert!(!state.snapshot(true).can_go_back);
        assert!(!state.snapshot(true).can_capture);
        assert!(state.capture_task.is_none());
    }

    #[test]
    fn navigation_preserves_ports_and_rejects_nonweb_schemes() {
        assert_eq!(
            normalize_address("localhost:3000/a").unwrap(),
            "http://localhost:3000/a"
        );
        assert_eq!(
            normalize_address("example.com/a?q=2").unwrap(),
            "https://example.com/a?q=2"
        );
        assert!(normalize_address("file:///etc/passwd").is_err());
        assert!(normalize_address("javascript:alert(1)").is_err());
        assert!(normalize_address("https://name:secret@example.com").is_err());
    }
    #[test]
    fn repeated_projection_does_not_repeat_navigation_and_observed_url_updates_address() {
        let mut state = IabPanelState::default();
        state.apply(IabAction::AddressChanged("example.com".into()));
        state.apply(IabAction::Navigate);
        let revision = state.revision;
        state.snapshot(false);
        state.snapshot(false);
        assert_eq!(state.revision, revision);
        state.receive(NativeBrowserEvent {
            id: BROWSER_ID.into(),
            node: nana_ui::runtime::StableNodeId::new(1).unwrap(),
            revision: state.revision,
            event: BrowserEvent::State(nana_ui::BrowserState {
                attached: true,
                url: "https://example.com/redirect".into(),
                ..Default::default()
            }),
        });
        assert_eq!(
            state.snapshot(false).address,
            "https://example.com/redirect"
        );
        assert!(state.browser_ready());
    }
}
