use lilia_contracts::{
    BrowserControl, BrowserHostDecision, BrowserHostRequest, BrowserHostRequestKind, BrowserScope,
};

#[derive(Clone, Debug, PartialEq)]
pub struct BrowserPresentation {
    pub resource: String,
    pub url: String,
    pub ready: bool,
    pub failed: bool,
    pub human_control: bool,
    pub busy: bool,
    pub status: String,
    pub requests: Vec<BrowserHostRequest>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BrowserAction {
    Navigate(String),
    Back,
    Forward,
    Reload,
    Cancel,
    Takeover,
    Resume,
    Retry,
    InstallRuntime,
    ApproveRequest(BrowserHostRequest),
    RejectRequest(BrowserHostRequest),
}

#[derive(Clone)]
pub struct BrowserView {
    pub root: nana_ui::runtime::Entity<nana_ui::runtime::Stack>,
    page: nana_ui::runtime::Entity<nana_ui::runtime::NativeContent>,
    address: nana_ui::runtime::Entity<nana_ui::runtime::TextInput>,
    status: nana_ui::runtime::Entity<nana_ui::runtime::Text>,
    status_row: nana_ui::runtime::Entity<nana_ui::runtime::Stack>,
    control: nana_ui::runtime::Entity<nana_ui::runtime::Button>,
    cancel: nana_ui::runtime::Entity<nana_ui::runtime::Button>,
    binding: std::sync::Arc<
        std::sync::Mutex<Option<(crate::runtime_shell::ShellPaneTarget, String, bool)>>,
    >,
    last_url: String,
    ready: bool,
    failed: bool,
    navigation: Vec<nana_ui::runtime::Entity<nana_ui::runtime::Button>>,
    document: nana_ui::runtime::DocumentId,
    sink: crate::runtime_shell::IntentSink,
    requests: nana_ui::runtime::Entity<nana_ui::runtime::Stack>,
    request_rows: Vec<nana_ui::runtime::Entity<nana_ui::runtime::Stack>>,
    last_requests: Vec<BrowserHostRequest>,
    toolbar: nana_ui::runtime::Entity<nana_ui::runtime::Stack>,
    recovery: nana_ui::runtime::Entity<nana_ui::runtime::Stack>,
}

impl BrowserView {
    #[cfg(debug_assertions)]
    pub(crate) fn debug_targets(
        &self,
        document: &nana_ui::runtime::RuntimeDocument,
    ) -> Vec<String> {
        let Some(bounds) = document.scene().node_bounds(self.root.stable_id()) else {
            return vec![];
        };
        if bounds.width <= 0.0
            || bounds.height <= 0.0
            || document
                .context()
                .world()
                .node(self.root.stable_id())
                .is_none_or(|node| node.parent.is_none())
        {
            return vec![];
        }
        let binding = self.binding.lock().unwrap();
        let Some((target, _, _)) = binding.as_ref() else {
            return vec![];
        };
        let mut actions = if self.ready {
            vec![
                "address".to_string(),
                "back".into(),
                "forward".into(),
                "reload".into(),
                "cancel".into(),
                "open".into(),
                "control".into(),
            ]
        } else {
            vec![]
        };
        if document
            .context()
            .world()
            .node(self.recovery.stable_id())
            .is_some_and(|node| node.parent.is_some())
        {
            actions.extend(["retry".into(), "install-runtime".into()]);
        }
        for request in &self.last_requests {
            actions.extend([
                format!("request.{}.approve", request.id),
                format!("request.{}.reject", request.id),
            ]);
        }
        actions
            .iter()
            .map(|action| {
                crate::target_ids::browser_control(
                    target.window_id.0,
                    &target.pane_id,
                    &target.item_id,
                    action,
                )
            })
            .collect()
    }

    #[cfg(debug_assertions)]
    pub(crate) fn debug_click(&self, id: &str) -> Option<crate::runtime_shell::ShellIntent> {
        let binding = self.binding.lock().unwrap();
        let (target, url, human) = binding.as_ref()?;
        let matches = |control: &str| {
            crate::target_ids::browser_control(
                target.window_id.0,
                &target.pane_id,
                &target.item_id,
                control,
            ) == id
        };
        let mut action = [
            ("back", BrowserAction::Back),
            ("forward", BrowserAction::Forward),
            ("reload", BrowserAction::Reload),
            ("cancel", BrowserAction::Cancel),
            ("open", BrowserAction::Navigate(url.clone())),
            (
                "control",
                if *human {
                    BrowserAction::Resume
                } else {
                    BrowserAction::Takeover
                },
            ),
            ("retry", BrowserAction::Retry),
            ("install-runtime", BrowserAction::InstallRuntime),
        ]
        .into_iter()
        .find_map(|(name, action)| matches(name).then_some(action));
        for request in &self.last_requests {
            if matches(&format!("request.{}.approve", request.id)) {
                action = Some(BrowserAction::ApproveRequest(request.clone()));
            }
            if matches(&format!("request.{}.reject", request.id)) {
                action = Some(BrowserAction::RejectRequest(request.clone()));
            }
        }
        let action = action?;
        let available = match &action {
            BrowserAction::Retry | BrowserAction::InstallRuntime => self.failed,
            BrowserAction::Cancel => self.ready,
            BrowserAction::ApproveRequest(_) | BrowserAction::RejectRequest(_) => true,
            _ => self.ready,
        };
        available.then(|| crate::runtime_shell::ShellIntent::Browser {
            target: target.clone(),
            action,
        })
    }

    #[cfg(debug_assertions)]
    pub(crate) fn debug_input(
        &self,
        id: &str,
        text: &str,
        context: &mut nana_ui::runtime::AppContext,
    ) -> bool {
        if !self.ready {
            return false;
        }
        let mut binding = self.binding.lock().unwrap();
        let Some((target, url, _)) = binding.as_mut() else {
            return false;
        };
        if crate::target_ids::browser_control(
            target.window_id.0,
            &target.pane_id,
            &target.item_id,
            "address",
        ) != id
        {
            return false;
        }
        if context
            .update_component(self.address, |address, _| {
                address.state.replace_value(text.to_owned());
            })
            .is_err()
        {
            return false;
        }
        *url = text.to_owned();
        true
    }

    pub fn dispose(
        self,
        context: &mut nana_ui::runtime::AppContext,
    ) -> Result<(), nana_ui::runtime::FrameworkError> {
        for view in [self.root, self.recovery, self.requests] {
            if context.world().contains(view.stable_id()) {
                context.remove_view(view)?;
            }
        }
        if context.world().contains(self.cancel.stable_id()) {
            context.remove_view(self.cancel)?;
        }
        Ok(())
    }
    pub fn mount(
        context: &mut nana_ui::runtime::AppContext,
        document: nana_ui::runtime::DocumentId,
        sink: &crate::runtime_shell::IntentSink,
    ) -> Result<Self, nana_ui::runtime::FrameworkError> {
        use nana_ui::runtime::*;
        let root = context.create_detached_component(document, Stack::fill_column(8.0))?;
        let toolbar = context.create_detached_component(document, Stack::bar(6.0))?;
        let mut address_input = TextInput::new("about:blank");
        let layout = std::sync::Arc::make_mut(&mut address_input.style.layout);
        layout.flex_grow = Some(1.0);
        layout.flex_shrink = Some(1.0);
        layout.min_width = Some(LengthSpec::Px(0.0));
        let address = context.create_detached_component(document, address_input)?;
        let mut status_text = Text::new("正在打开浏览器…");
        let status_layout = std::sync::Arc::make_mut(&mut status_text.style.layout);
        status_layout.flex_grow = Some(1.0);
        status_layout.flex_shrink = Some(1.0);
        status_layout.min_width = Some(LengthSpec::Px(0.0));
        let status = context.create_detached_component(document, status_text)?;
        let page = context.create_detached_component(
            document,
            NativeContent::new("browser-pending").attached(false),
        )?;
        let binding = std::sync::Arc::new(std::sync::Mutex::new(
            None::<(crate::runtime_shell::ShellPaneTarget, String, bool)>,
        ));
        let mut navigation = Vec::new();
        for (label, action) in [
            ("后退", BrowserAction::Back),
            ("前进", BrowserAction::Forward),
            ("刷新", BrowserAction::Reload),
            ("打开", BrowserAction::Navigate(String::new())),
        ] {
            let button = context.create_detached_component(document, Button::new(label))?;
            let state = binding.clone();
            let sink = sink.clone();
            context.on(button, move |_, _: &Activate, _| {
                if let Some((target, url, _)) = state.lock().unwrap().as_ref() {
                    let action = if matches!(action, BrowserAction::Navigate(_)) {
                        BrowserAction::Navigate(url.clone())
                    } else {
                        action.clone()
                    };
                    crate::runtime_shell::emit(
                        &sink,
                        crate::runtime_shell::ShellIntent::Browser {
                            target: target.clone(),
                            action,
                        },
                    );
                }
            })?;
            navigation.push(button);
            if label == "打开" {
                context.append_child(toolbar, address)?;
            }
            context.append_child(toolbar, button)?;
        }
        let state = binding.clone();
        context.on(address, move |_, event: &TextChanged, _| {
            if let Some((_, url, _)) = state.lock().unwrap().as_mut() {
                *url = event.value.clone();
            }
        })?;
        let control = context.create_detached_component(document, Button::new("接管"))?;
        let cancel = context.create_detached_component(document, Button::new("停止操作"))?;
        let cancel_state = binding.clone();
        let cancel_sink = sink.clone();
        context.on(cancel, move |_, _: &Activate, _| {
            if let Some((target, _, _)) = cancel_state.lock().unwrap().as_ref() {
                crate::runtime_shell::emit(
                    &cancel_sink,
                    crate::runtime_shell::ShellIntent::Browser {
                        target: target.clone(),
                        action: BrowserAction::Cancel,
                    },
                );
            }
        })?;
        let state = binding.clone();
        let control_sink = sink.clone();
        context.on(control, move |_, _: &Activate, _| {
            if let Some((target, _, human)) = state.lock().unwrap().as_ref() {
                crate::runtime_shell::emit(
                    &control_sink,
                    crate::runtime_shell::ShellIntent::Browser {
                        target: target.clone(),
                        action: if *human {
                            BrowserAction::Resume
                        } else {
                            BrowserAction::Takeover
                        },
                    },
                );
            }
        })?;
        let status_row = context.create_detached_component(
            document,
            Stack::bar(8.0).justify(JustifySpec::SpaceBetween),
        )?;
        context.append_child(status_row, status)?;
        context.append_child(status_row, control)?;
        context.append_child(status_row, cancel)?;
        let recovery = context.create_detached_component(document, Stack::row(8.0))?;
        for (label, action) in [
            ("重试", BrowserAction::Retry),
            ("安装浏览组件", BrowserAction::InstallRuntime),
        ] {
            let button = context.create_detached_component(document, Button::new(label))?;
            let state = binding.clone();
            let sink = sink.clone();
            context.on(button, move |_, _: &Activate, _| {
                if let Some((target, _, _)) = state.lock().unwrap().as_ref() {
                    crate::runtime_shell::emit(
                        &sink,
                        crate::runtime_shell::ShellIntent::Browser {
                            target: target.clone(),
                            action: action.clone(),
                        },
                    );
                }
            })?;
            context.append_child(recovery, button)?;
        }
        let requests = context.create_detached_component(document, Stack::column(6.0))?;
        context.append_child(root, toolbar)?;
        context.append_child(root, status_row)?;
        context.append_child(root, page)?;
        Ok(Self {
            root,
            page,
            address,
            status,
            status_row,
            control,
            cancel,
            binding,
            last_url: String::new(),
            ready: false,
            failed: false,
            navigation,
            toolbar,
            recovery,
            document,
            sink: sink.clone(),
            requests,
            request_rows: vec![],
            last_requests: vec![],
        })
    }

    pub fn sync(
        &mut self,
        context: &mut nana_ui::runtime::AppContext,
        target: crate::runtime_shell::ShellPaneTarget,
        snapshot: &BrowserPresentation,
    ) -> Result<(), nana_ui::runtime::FrameworkError> {
        let mut binding = self.binding.lock().unwrap();
        let changed = binding
            .as_ref()
            .is_none_or(|(old, _, _)| old.item_id != target.item_id);
        let url = if changed || self.last_url != snapshot.url {
            context.update_component(self.address, |address, _| {
                address.state.replace_value(snapshot.url.clone());
            })?;
            self.last_url = snapshot.url.clone();
            snapshot.url.clone()
        } else {
            binding
                .as_ref()
                .map_or(snapshot.url.clone(), |(_, url, _)| url.clone())
        };
        *binding = Some((target, url, snapshot.human_control));
        drop(binding);
        self.sync_requests(context, &snapshot.requests)?;
        self.ready = snapshot.ready;
        self.failed = snapshot.failed;
        context.update_component(self.address, |address, _| {
            address.disabled = !snapshot.ready;
        })?;
        for button in &self.navigation {
            context.update_component(*button, |button, _| {
                button.disabled = !snapshot.ready;
            })?;
        }
        context.update_component(self.page, |page, _| {
            page.resource = snapshot.resource.clone().into();
            page.attached = snapshot.ready;
        })?;
        context.update_component(self.status, |status, _| {
            status.value = snapshot.status.clone();
        })?;
        context.update_component(self.control, |button, _| {
            *button = nana_ui::runtime::Button::new(if snapshot.human_control {
                "恢复 Agent"
            } else {
                "接管"
            });
            button.disabled = !snapshot.ready;
        })?;
        context.update_component(self.cancel, |button, _| {
            *button = nana_ui::runtime::Button::new("停止操作");
            button.disabled = !snapshot.busy;
        })?;
        let mut children = vec![self.toolbar.stable_id(), self.status_row.stable_id()];
        if snapshot.failed {
            children.push(self.recovery.stable_id());
        }
        if !snapshot.requests.is_empty() {
            children.push(self.requests.stable_id());
        }
        children.push(self.page.stable_id());
        crate::runtime_layout::reconcile_children(context, self.root.stable_id(), &children)?;
        Ok(())
    }

    fn sync_requests(
        &mut self,
        context: &mut nana_ui::runtime::AppContext,
        requests: &[BrowserHostRequest],
    ) -> Result<(), nana_ui::runtime::FrameworkError> {
        use nana_ui::runtime::*;
        if self.last_requests == requests {
            return Ok(());
        }
        for row in self.request_rows.drain(..) {
            context.remove_view(row)?;
        }
        for request in requests {
            let row = context.create_detached_component(
                self.document,
                Stack::column(6.0)
                    .padding(8.0)
                    .surface(SemanticColorRole::Surface)
                    .outline(SemanticColorRole::Border, 1.0)
                    .radius(8.0),
            )?;
            let (message, approve) = match &request.kind {
                BrowserHostRequestKind::Download { suggested_filename } => (
                    format!("网页请求下载：{suggested_filename}"),
                    "选择保存位置",
                ),
                BrowserHostRequestKind::Upload { multiple } => (
                    "网页请求上传文件".into(),
                    if *multiple {
                        "选择文件"
                    } else {
                        "选择一个文件"
                    },
                ),
                BrowserHostRequestKind::NewWindow { url } => {
                    (format!("网页请求打开新标签页：{url}"), "打开标签页")
                }
            };
            let text = context.create_detached_component(self.document, Text::new(message))?;
            context.append_child(row, text)?;
            let actions = context.create_detached_component(self.document, Stack::row(6.0))?;
            for (label, allow) in [(approve, true), ("拒绝", false)] {
                let button =
                    context.create_detached_component(self.document, Button::new(label))?;
                let binding = self.binding.clone();
                let sink = self.sink.clone();
                let ticket = request.clone();
                context.on(button, move |_, _: &Activate, _| {
                    if let Some((target, _, _)) = binding.lock().unwrap().as_ref() {
                        if target.item_id.as_str() != ticket.scope.tab_id {
                            return;
                        }
                        crate::runtime_shell::emit(
                            &sink,
                            crate::runtime_shell::ShellIntent::Browser {
                                target: target.clone(),
                                action: if allow {
                                    BrowserAction::ApproveRequest(ticket.clone())
                                } else {
                                    BrowserAction::RejectRequest(ticket.clone())
                                },
                            },
                        );
                    }
                })?;
                context.append_child(actions, button)?;
            }
            context.append_child(row, actions)?;
            context.append_child(self.requests, row)?;
            self.request_rows.push(row);
        }
        self.last_requests = requests.to_vec();
        Ok(())
    }
}

#[cfg(debug_assertions)]
#[derive(Default)]
pub(crate) struct BrowserDebugStats {
    pub states: Vec<serde_json::Value>,
    pub visible: bool,
    pub attached: bool,
    pub ready: bool,
    pub url: String,
    pub error: Option<String>,
    pub count: usize,
    pub ready_count: usize,
    pub task_ids: Vec<String>,
    pub urls: Vec<String>,
    pub capture_pending: usize,
    pub notice: Option<String>,
}

#[cfg(target_os = "windows")]
pub struct BrowserWorkbench {
    host: crate::platform::browser::UiBrowserHost,
    sessions: std::sync::Arc<lilia_agent::BrowserSessions>,
    scopes: std::collections::BTreeMap<String, BrowserScope>,
    visuals: std::collections::BTreeMap<
        String,
        (nana_ui_platform::WindowId, nana_ui::WindowsNativeVisual),
    >,
    errors: std::collections::BTreeMap<String, String>,
    notices: std::collections::BTreeMap<String, String>,
    choosing: std::collections::BTreeMap<u64, BrowserHostRequest>,
    requested_tabs: std::collections::BTreeMap<String, BrowserHostRequest>,
    regions: std::collections::BTreeMap<
        nana_ui_platform::WindowId,
        (f32, Vec<nana_ui::NativeContentRegion>),
    >,
    selected: std::cell::RefCell<std::collections::BTreeMap<lilia_contracts::TaskId, String>>,
    opened: Vec<String>,
    restore_urls: std::collections::BTreeMap<String, String>,
    captured: std::cell::RefCell<std::collections::BTreeMap<nana_ui_platform::WindowId, String>>,
    hovered: std::cell::RefCell<std::collections::BTreeMap<nana_ui_platform::WindowId, String>>,
    home: std::path::PathBuf,
    wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    pending_wake: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(target_os = "windows")]
impl BrowserWorkbench {
    pub fn new(
        home: std::path::PathBuf,
        wake: std::sync::Arc<dyn Fn() + Send + Sync>,
        authority: std::sync::Arc<dyn lilia_agent::BrowserScopeAuthority>,
    ) -> Self {
        let pending_wake = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let pending = pending_wake.clone();
        let wake: std::sync::Arc<dyn Fn() + Send + Sync> = std::sync::Arc::new(move || {
            if !pending.swap(true, std::sync::atomic::Ordering::AcqRel) {
                wake();
            }
        });
        let (host, bridge) = crate::platform::browser::UiBrowserHost::new(wake.clone());
        let sessions = std::sync::Arc::new(lilia_agent::BrowserSessions::with_authority(
            bridge, authority,
        ));
        host.attach_sessions(&sessions);
        Self {
            host,
            sessions,
            scopes: Default::default(),
            visuals: Default::default(),
            errors: Default::default(),
            notices: Default::default(),
            choosing: Default::default(),
            requested_tabs: Default::default(),
            regions: Default::default(),
            selected: Default::default(),
            opened: vec![],
            restore_urls: Default::default(),
            captured: Default::default(),
            hovered: Default::default(),
            home,
            wake,
            pending_wake,
        }
    }

    #[cfg(debug_assertions)]
    pub(crate) fn debug_stats(&self) -> BrowserDebugStats {
        let visible: std::collections::BTreeSet<_> = self
            .regions
            .values()
            .flat_map(|(_, regions)| regions.iter().map(|region| region.resource.to_string()))
            .collect();
        let states: Vec<_> = self
            .scopes
            .values()
            .filter_map(|scope| self.sessions.state(scope).ok())
            .collect();
        let current = states
            .iter()
            .find(|state| visible.contains(&state.scope.tab_id));
        let safe_url = |value: &str| {
            url::Url::parse(value)
                .map(|mut url| {
                    let _ = url.set_username("");
                    let _ = url.set_password(None);
                    url.set_query(None);
                    url.set_fragment(None);
                    url.to_string()
                })
                .unwrap_or_default()
        };
        BrowserDebugStats {
            states: states.iter().map(|state| serde_json::json!({
                "scope": state.scope, "lifecycle": state.lifecycle, "pageVersion": state.page_version,
                "control": state.control, "url": safe_url(&state.page.url),
            })).collect(),
            visible: !visible.is_empty(),
            attached: visible.iter().any(|key| self.visuals.contains_key(key)),
            ready: current.is_some(),
            url: current.map_or_else(|| "about:blank".into(), |state| safe_url(&state.page.url)),
            error: (!self.errors.is_empty()).then(|| "浏览器未能打开".into()),
            count: self.scopes.len(),
            ready_count: states.len(),
            task_ids: self
                .scopes
                .values()
                .map(|scope| scope.task_id.as_str().to_string())
                .collect(),
            urls: self
                .scopes
                .values()
                .map(|scope| {
                    self.sessions
                        .state(scope)
                        .ok()
                        .map_or_else(|| "about:blank".into(), |state| safe_url(&state.page.url))
                })
                .collect(),
            capture_pending: self.host.capture_pending_count(),
            notice: self.notices.values().next().cloned(),
        }
    }

    pub fn sessions(&self) -> std::sync::Arc<lilia_agent::BrowserSessions> {
        self.sessions.clone()
    }

    pub fn retain(&mut self, resources: &std::collections::HashSet<String>) {
        let closed: Vec<_> = self
            .scopes
            .keys()
            .filter(|key| !resources.contains(*key))
            .cloned()
            .collect();
        for key in closed {
            if let Some(scope) = self.scopes.remove(&key) {
                let _ = self.host.set_pointer_capture(&scope, false);
                let _ = self.host.close(&scope);
                self.opened.retain(|resource| resource != &key);
                if self.selected.borrow().get(&scope.task_id) == Some(&key) {
                    let fallback = self
                        .opened
                        .iter()
                        .rev()
                        .find(|resource| {
                            self.scopes
                                .get(*resource)
                                .is_some_and(|candidate| candidate.task_id == scope.task_id)
                        })
                        .cloned();
                    if let Some(fallback) = fallback {
                        self.selected
                            .borrow_mut()
                            .insert(scope.task_id.clone(), fallback);
                    } else {
                        self.selected.borrow_mut().remove(&scope.task_id);
                    }
                }
            }
            self.captured
                .borrow_mut()
                .retain(|_, resource| resource != &key);
            self.hovered
                .borrow_mut()
                .retain(|_, resource| resource != &key);
            if let Some(request) = self.requested_tabs.remove(&key) {
                let _ = self.host.respond(&request, BrowserHostDecision::Reject);
            }
            self.choosing
                .retain(|_, request| request.scope.tab_id != key);
            self.notices.remove(&key);
            self.visuals.remove(&key);
            self.errors.remove(&key);
            self.restore_urls.remove(&key);
        }
    }

    pub fn open(
        &mut self,
        scope: BrowserScope,
    ) -> Result<crate::application::WorkspaceItem, String> {
        self.sessions
            .validate_scope(&scope)
            .map_err(|_| "此任务的浏览器当前不可用。".to_string())?;
        let resource = scope.tab_id.clone();
        if self
            .scopes
            .get(&resource)
            .is_some_and(|previous| previous != &scope)
        {
            let retained = self
                .scopes
                .keys()
                .filter(|key| *key != &resource)
                .cloned()
                .collect();
            self.retain(&retained);
        }
        let item =
            crate::application::browser_workspace_item(&lilia_contracts::BrowserTabRestoration {
                scope: scope.clone(),
                url: self
                    .restoration(&resource)
                    .map_or("about:blank".into(), |state| state.url),
            })
            .map_err(|error| error.to_string())?;
        self.selected
            .borrow_mut()
            .insert(scope.task_id.clone(), resource.clone());
        if !self.opened.contains(&resource) {
            self.opened.push(resource.clone());
        }
        self.scopes.insert(resource, scope);
        Ok(item)
    }

    pub fn restore_items(&mut self, items: &[crate::application::WorkspaceItem]) {
        use crate::application::WorkspaceItemResolve;
        for item in items {
            let Ok(Some(state)) = item.browser_restoration() else {
                continue;
            };
            if self.scopes.contains_key(&state.scope.tab_id) {
                continue;
            }
            let resource = state.scope.tab_id.clone();
            self.selected
                .borrow_mut()
                .entry(state.scope.task_id.clone())
                .or_insert(resource.clone());
            self.opened.push(resource.clone());
            self.restore_urls.insert(resource.clone(), state.url);
            self.scopes.insert(resource, state.scope);
        }
    }

    pub fn restoration(&self, resource: &str) -> Option<lilia_contracts::BrowserTabRestoration> {
        let scope = self.scopes.get(resource)?.clone();
        let current = self.sessions.state(&scope).ok().map(|state| state.page.url);
        let url = current
            .filter(|url| url != "about:blank")
            .or_else(|| self.restore_urls.get(resource).cloned())
            .unwrap_or_else(|| "about:blank".into());
        Some(lilia_contracts::BrowserTabRestoration { scope, url })
    }

    pub fn presentation(&self, resource: &str) -> Option<BrowserPresentation> {
        let scope = self.scopes.get(resource)?;
        let state = self.sessions.state(scope).ok();
        Some(BrowserPresentation {
            resource: resource.into(),
            url: self.restoration(resource)?.url,
            ready: state.is_some(),
            failed: self.errors.contains_key(resource),
            human_control: state
                .as_ref()
                .is_some_and(|state| state.control == BrowserControl::Human),
            busy: self.sessions.is_busy(scope),
            requests: self
                .host
                .pending_requests()
                .into_iter()
                .filter(|request| {
                    &request.scope == scope
                        && !self.choosing.contains_key(&request.id)
                        && !self
                            .requested_tabs
                            .values()
                            .any(|pending| pending == request)
                })
                .collect(),
            status: if self
                .choosing
                .values()
                .any(|request| &request.scope == scope)
            {
                "正在处理网页请求…".into()
            } else if let Some(notice) = self.notices.get(resource) {
                notice.clone()
            } else if self.errors.contains_key(resource) {
                "网页未能打开，请重新打开浏览器。".into()
            } else if state.is_none() {
                "正在打开浏览器…".into()
            } else if state
                .as_ref()
                .is_some_and(|state| state.control == BrowserControl::Human)
            {
                "你正在操作 · Agent 已暂停".into()
            } else {
                "Agent 可以操作此页面".into()
            },
        })
    }

    pub fn poll(&mut self) -> bool {
        use crate::platform::browser::BrowserHostEvent;
        self.pending_wake
            .store(false, std::sync::atomic::Ordering::Release);
        self.host.poll();
        let events = self.host.drain_events();
        let changed = !events.is_empty();
        for event in events {
            match event {
                BrowserHostEvent::Ready(scope) => {
                    self.errors.remove(&scope.tab_id);
                    if let Some(url) = self.restore_urls.get(&scope.tab_id) {
                        let _ = self.sessions.takeover(&scope);
                        if url != "about:blank" {
                            if let Err(error) = self.host.navigate(&scope, url) {
                                self.errors.insert(scope.tab_id.clone(), error.to_string());
                            }
                        }
                    }
                    if let Some(request) = self.requested_tabs.remove(&scope.tab_id) {
                        if self
                            .host
                            .respond(
                                &request,
                                BrowserHostDecision::NewWindow {
                                    target_scope: scope.clone(),
                                },
                            )
                            .is_err()
                        {
                            let _ = self.host.respond(&request, BrowserHostDecision::Reject);
                            let _ = self.host.close(&scope);
                            self.errors.insert(
                                scope.tab_id.clone(),
                                "新标签页请求已失效，请从原页面重新打开。".into(),
                            );
                        }
                    }
                }
                BrowserHostEvent::Failed(scope, error) => {
                    if let Some(request) = self.requested_tabs.remove(&scope.tab_id) {
                        let _ = self.host.respond(&request, BrowserHostDecision::Reject);
                    }
                    self.errors.insert(scope.tab_id, error);
                }
                BrowserHostEvent::Request(request) => {
                    self.notices.remove(&request.scope.tab_id);
                }
                BrowserHostEvent::RequestResolved { request, error } => {
                    self.choosing.remove(&request.id);
                    self.notices.insert(
                        request.scope.tab_id.clone(),
                        if error.is_some() {
                            "网页请求未完成，请从页面重试。".into()
                        } else {
                            "网页请求已处理".into()
                        },
                    );
                    let abandoned: Vec<_> = self
                        .requested_tabs
                        .iter()
                        .filter(|(_, pending)| **pending == request)
                        .map(|(resource, _)| resource.clone())
                        .collect();
                    for resource in abandoned {
                        self.requested_tabs.remove(&resource);
                        if let Some(scope) = self.scopes.get(&resource) {
                            let _ = self.host.close(scope);
                        }
                        self.errors
                            .insert(resource, "新标签页请求已取消，请从原页面重新打开。".into());
                    }
                }
            }
        }
        self.restore_urls.retain(|resource, _| {
            self.scopes
                .get(resource)
                .and_then(|scope| self.sessions.state(scope).ok())
                .is_none_or(|state| state.page.url == "about:blank")
        });
        changed
    }

    pub fn action(&mut self, resource: &str, action: BrowserAction) -> Result<(), String> {
        let scope = self.scopes.get(resource).ok_or("浏览器已关闭")?;
        self.selected
            .borrow_mut()
            .insert(scope.task_id.clone(), resource.to_owned());
        if action == BrowserAction::Retry {
            if let Some(state) = self.restoration(resource) {
                self.restore_urls.insert(resource.to_owned(), state.url);
            }
            let _ = self.host.close(scope);
            self.visuals.remove(resource);
            self.errors.remove(resource);
            return Ok(());
        }
        let result = match action {
            BrowserAction::Navigate(url) => {
                let result = self.host.navigate(scope, &url);
                if result.is_ok() {
                    self.restore_urls.remove(resource);
                }
                result
            }
            BrowserAction::Back => self.host.back(scope),
            BrowserAction::Forward => self.host.forward(scope),
            BrowserAction::Reload => self.host.reload(scope),
            BrowserAction::Cancel => self.sessions.cancel(scope).map(|_| ()),
            BrowserAction::Takeover => self.sessions.takeover(scope).map(|_| ()),
            BrowserAction::Resume => self.sessions.resume(scope).map(|_| ()),
            BrowserAction::RejectRequest(request) => {
                if request.scope != *scope {
                    return Err("网页请求不属于当前标签页".into());
                }
                return self.respond(&request, BrowserHostDecision::Reject);
            }
            BrowserAction::ApproveRequest(_) => return Err("请从请求卡选择文件或打开标签页".into()),
            BrowserAction::Retry | BrowserAction::InstallRuntime => return Ok(()),
        };
        result.map_err(|_| "暂时无法完成浏览操作，请检查页面后重试。".into())
    }

    pub fn begin_request(&mut self, request: &BrowserHostRequest) -> Result<(), String> {
        self.sessions
            .validate_scope(&request.scope)
            .map_err(|_| "网页请求已失效，请从页面重试。".to_string())?;
        if self.scopes.get(&request.scope.tab_id) != Some(&request.scope)
            || !self.host.pending_requests().contains(request)
        {
            return Err("网页请求已失效，请从页面重试。".into());
        }
        if self.choosing.contains_key(&request.id)
            || self
                .requested_tabs
                .values()
                .any(|pending| pending == request)
        {
            return Err("正在处理此请求".into());
        }
        self.choosing.insert(request.id, request.clone());
        (self.wake)();
        Ok(())
    }

    pub fn respond(
        &mut self,
        request: &BrowserHostRequest,
        decision: BrowserHostDecision,
    ) -> Result<(), String> {
        self.choosing.remove(&request.id);
        if self.scopes.get(&request.scope.tab_id) != Some(&request.scope) {
            return Err("浏览器已关闭".into());
        }
        (self.wake)();
        self.host
            .respond(request, decision)
            .map_err(|_| "网页请求已失效或选择无法使用，请重试。".to_string())?;
        self.notices.remove(&request.scope.tab_id);
        (self.wake)();
        Ok(())
    }

    pub fn open_requested(
        &mut self,
        request: &BrowserHostRequest,
    ) -> Result<crate::application::WorkspaceItem, String> {
        self.choosing.remove(&request.id);
        if !matches!(request.kind, BrowserHostRequestKind::NewWindow { .. })
            || self.scopes.get(&request.scope.tab_id) != Some(&request.scope)
            || !self.host.pending_requests().contains(request)
        {
            return Err("新标签页请求已失效".into());
        }
        if self
            .requested_tabs
            .values()
            .any(|pending| pending == request)
        {
            return Err("正在打开标签页".into());
        }
        let mut scope = request.scope.clone();
        scope.tab_id = format!(
            "browser:{}:{}",
            scope.task_id.as_str(),
            uuid::Uuid::new_v4()
        );
        let item = self.open(scope.clone())?;
        self.requested_tabs.insert(scope.tab_id, request.clone());
        Ok(item)
    }

    pub fn frame(
        &mut self,
        id: nana_ui_platform::WindowId,
        composition: &nana_ui::WindowsComposition,
        regions: &[nana_ui::NativeContentRegion],
        scale: f32,
    ) -> Result<(), String> {
        use windows::{
            Win32::{
                Foundation::{HWND, RECT},
                Graphics::DirectComposition::IDCompositionVisual,
            },
            core::Interface,
        };
        self.regions.insert(id, (scale, regions.to_vec()));
        for (resource, (window, visual)) in &self.visuals {
            if *window == id {
                visual
                    .set_visible(false)
                    .map_err(|error| error.to_string())?;
                if !regions
                    .iter()
                    .any(|region| region.resource.as_ref() == resource)
                {
                    if let Some(scope) = self.scopes.get(resource) {
                        let _ = self.host.set_pointer_capture(scope, false);
                        self.captured
                            .borrow_mut()
                            .retain(|_, value| value != resource);
                        self.hovered
                            .borrow_mut()
                            .retain(|_, value| value != resource);
                        let _ = self.host.set_visible(scope, false);
                    }
                }
            }
        }
        for region in regions {
            let scope = self
                .scopes
                .get(region.resource.as_ref())
                .ok_or("unknown browser region")?;
            let needs_visual = self
                .visuals
                .get(region.resource.as_ref())
                .is_none_or(|(window, _)| *window != id);
            if needs_visual {
                let visual = composition
                    .create_native_visual()
                    .map_err(|e| e.to_string())?;
                // The framework owns the borrowed COM visual; clone its reference for WebView2.
                let raw = visual.as_raw();
                let target = unsafe { IDCompositionVisual::from_raw_borrowed(&raw) }
                    .ok_or("invalid composition visual")?
                    .clone();
                if self.visuals.contains_key(region.resource.as_ref()) {
                    self.host
                        .reparent(scope, HWND(composition.window_handle()), target)
                        .map_err(|e| e.to_string())?;
                } else {
                    let created =
                        if let Some(request) = self.requested_tabs.get(region.resource.as_ref()) {
                            self.host.create_for_request(
                                request,
                                scope.clone(),
                                HWND(composition.window_handle()),
                                target,
                                &self.home.join("browser-profiles"),
                            )
                        } else {
                            self.host.create(
                                scope.clone(),
                                HWND(composition.window_handle()),
                                target,
                                &self.home.join("browser-profiles"),
                            )
                        };
                    if let Err(error) = created {
                        self.errors
                            .insert(region.resource.to_string(), error.to_string());
                        if let Some(request) = self.requested_tabs.remove(region.resource.as_ref())
                        {
                            let _ = self.host.respond(&request, BrowserHostDecision::Reject);
                        }
                        (self.wake)();
                    }
                }
                self.visuals
                    .insert(region.resource.to_string(), (id, visual));
            }
            let visual = &self.visuals[region.resource.as_ref()].1;
            let bounds = nana_ui::WindowsCompositionRect {
                x: region.bounds.x * scale,
                y: region.bounds.y * scale,
                width: region.bounds.width * scale,
                height: region.bounds.height * scale,
            };
            let clip = nana_ui::WindowsCompositionRect {
                x: region.clip.x * scale,
                y: region.clip.y * scale,
                width: region.clip.width * scale,
                height: region.clip.height * scale,
            };
            visual
                .set_geometry(bounds, Some(clip))
                .map_err(|e| e.to_string())?;
            if self.sessions.state(scope).is_ok() {
                self.host
                    .set_rasterization_scale(scope, f64::from(scale))
                    .map_err(|e| e.to_string())?;
                self.host
                    .set_bounds(
                        scope,
                        RECT {
                            left: bounds.x as i32,
                            top: bounds.y as i32,
                            right: (bounds.x + bounds.width) as i32,
                            bottom: (bounds.y + bounds.height) as i32,
                        },
                        true,
                    )
                    .map_err(|e| e.to_string())?;
                visual.set_visible(true).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn bind_agents(&self, runtime: &lilia_agent::NativeAgentKitRuntime) {
        for resource in self.selected.borrow().values() {
            let Some(scope) = self.scopes.get(resource) else {
                continue;
            };
            if self.sessions.state(scope).is_ok() {
                if self.sessions.select_agent_scope(scope).is_err() {
                    continue;
                }
                for session in runtime.session_ids_for_task(&scope.task_id) {
                    let _ = runtime.bind_browser_session(&session, scope.clone());
                }
            }
        }
    }

    pub fn input(
        &self,
        id: nana_ui_platform::WindowId,
        event: &nana_ui_platform::InputEvent,
        hit: Option<nana_ui::runtime::StableNodeId>,
    ) -> bool {
        use nana_ui_platform::{InputEvent, PointerPhase};
        use webview2_com::Microsoft::Web::WebView2::Win32::*;
        use windows::Win32::Foundation::POINT;
        let Some((scale, regions)) = self.regions.get(&id) else {
            return false;
        };
        let hit_region = regions.iter().find(|region| Some(region.node) == hit);
        let capture = self.captured.borrow().get(&id).cloned();
        let pointer = matches!(event, InputEvent::Pointer { .. });
        if pointer && capture.is_none() {
            let previous = self.hovered.borrow_mut().remove(&id);
            let current = hit_region.map(|region| region.resource.to_string());
            if previous != current {
                if let Some(scope) = previous
                    .as_ref()
                    .and_then(|resource| self.scopes.get(resource))
                {
                    let _ = self.host.mouse_input(
                        scope,
                        COREWEBVIEW2_MOUSE_EVENT_KIND_LEAVE,
                        COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS(0),
                        0,
                        POINT::default(),
                    );
                }
            }
            if let Some(current) = current {
                self.hovered.borrow_mut().insert(id, current);
            }
        }
        if matches!(
            event,
            InputEvent::Pointer {
                phase: PointerPhase::Down,
                ..
            }
        ) && hit_region.is_none()
            && capture.is_none()
        {
            for region in regions {
                if let Some(scope) = self.scopes.get(region.resource.as_ref()) {
                    let _ = self.host.blur(scope);
                }
            }
        }
        let region = if pointer {
            capture
                .as_ref()
                .and_then(|resource| {
                    regions
                        .iter()
                        .find(|region| region.resource.as_ref() == resource)
                })
                .or(hit_region)
        } else {
            hit_region
        };
        let Some(region) = region else {
            return false;
        };
        let Some(scope) = self.scopes.get(region.resource.as_ref()) else {
            return false;
        };
        let (x, y, kind, data, modifiers, buttons) = match event {
            InputEvent::Pointer {
                x,
                y,
                phase,
                button,
                buttons,
                modifiers,
                ..
            } => {
                let kind = match (phase, button) {
                    (PointerPhase::Down, 0) => COREWEBVIEW2_MOUSE_EVENT_KIND_LEFT_BUTTON_DOWN,
                    (PointerPhase::Up, 0) => COREWEBVIEW2_MOUSE_EVENT_KIND_LEFT_BUTTON_UP,
                    (PointerPhase::Down, 2) => COREWEBVIEW2_MOUSE_EVENT_KIND_RIGHT_BUTTON_DOWN,
                    (PointerPhase::Up, 2) => COREWEBVIEW2_MOUSE_EVENT_KIND_RIGHT_BUTTON_UP,
                    (PointerPhase::Down, 1) => COREWEBVIEW2_MOUSE_EVENT_KIND_MIDDLE_BUTTON_DOWN,
                    (PointerPhase::Up, 1) => COREWEBVIEW2_MOUSE_EVENT_KIND_MIDDLE_BUTTON_UP,
                    (PointerPhase::Move, _) => COREWEBVIEW2_MOUSE_EVENT_KIND_MOVE,
                    (PointerPhase::Cancel, _) => COREWEBVIEW2_MOUSE_EVENT_KIND_LEAVE,
                    _ => return false,
                };
                if *phase == PointerPhase::Down {
                    self.selected
                        .borrow_mut()
                        .insert(scope.task_id.clone(), scope.tab_id.clone());
                    let _ = self.host.focus(scope);
                    if self.host.set_pointer_capture(scope, true).is_ok() {
                        self.captured.borrow_mut().insert(id, scope.tab_id.clone());
                    }
                }
                if *phase == PointerPhase::Cancel || (*phase == PointerPhase::Up && *buttons == 0) {
                    self.captured.borrow_mut().remove(&id);
                    let _ = self.host.set_pointer_capture(scope, false);
                }
                (*x, *y, kind, 0, *modifiers, *buttons)
            }
            InputEvent::Wheel {
                x,
                y,
                delta_y,
                line_delta,
                modifiers,
                ..
            } => (
                *x,
                *y,
                COREWEBVIEW2_MOUSE_EVENT_KIND_WHEEL,
                (-delta_y * if *line_delta { 120.0 } else { 1.0 }) as i32 as u32,
                *modifiers,
                0,
            ),
            _ => return false,
        };
        let bits = (if buttons & 1 != 0 { 1 } else { 0 })
            | (if buttons & 2 != 0 { 2 } else { 0 })
            | (if buttons & 4 != 0 { 16 } else { 0 })
            | (if modifiers.shift { 4 } else { 0 })
            | (if modifiers.control { 8 } else { 0 });
        self.host
            .mouse_input(
                scope,
                kind,
                COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS(bits),
                data,
                POINT {
                    x: ((x - region.bounds.x) * scale) as i32,
                    y: ((y - region.bounds.y) * scale) as i32,
                },
            )
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_toolbar_keeps_navigation_and_control_inside_the_view() {
        use nana_ui::runtime::{DocumentId, LayoutViewport, RuntimeDocument, Stack};
        let id = DocumentId::new(99302).unwrap();
        let mut document = RuntimeDocument::new(id);
        let root = document
            .context_mut()
            .create_component(id, Stack::fill_column(0.0))
            .unwrap();
        let sink: crate::runtime_shell::IntentSink = std::sync::Arc::new(|_| {});
        let mut view = BrowserView::mount(document.context_mut(), id, &sink).unwrap();
        document
            .context_mut()
            .append_child(root, view.root)
            .unwrap();
        let target = crate::runtime_shell::ShellPaneTarget {
            window_id: crate::runtime_compat::HostedWindowId::PRIMARY,
            pane_id: "pane".into(),
            item_id: "browser-tab".into(),
        };
        for width in [400.0, 760.0] {
            for (human_control, busy) in [(false, false), (true, false), (false, true)] {
                let presentation = BrowserPresentation {
                    resource: "browser-tab".into(),
                    url: format!("https://example.com/{}", "long-address-segment/".repeat(12)),
                    ready: true,
                    failed: false,
                    human_control,
                    busy,
                    status: "你正在操作 · Agent 已暂停".into(),
                    requests: vec![],
                };
                view.sync(document.context_mut(), target.clone(), &presentation)
                    .unwrap();
                document
                    .flush(
                        LayoutViewport::new(width, 640.0),
                        &mut nana_ui::NanaTextShaper::default(),
                    )
                    .unwrap();
                let parent = document.scene().node_bounds(view.root.stable_id()).unwrap();
                for node in view
                    .navigation
                    .iter()
                    .map(|node| node.stable_id())
                    .chain([view.address.stable_id(), view.control.stable_id()])
                {
                    let bounds = document.scene().node_bounds(node).unwrap();
                    assert!(
                        bounds.width >= 24.0
                            && bounds.x >= parent.x
                            && bounds.x + bounds.width <= parent.x + parent.width + 1.0,
                        "control {node:?} {bounds:?} overflows browser {parent:?}"
                    );
                }
                let control = document
                    .scene()
                    .node_bounds(view.control.stable_id())
                    .unwrap();
                assert!(
                    control.width >= if human_control { 70.0 } else { 35.0 },
                    "control label truncated: {control:?}"
                );
                let cancel_disabled = document
                    .context()
                    .read(view.cancel, |button| button.disabled)
                    .unwrap();
                assert_eq!(cancel_disabled, !busy);
            }
        }
    }

    #[test]
    fn resolved_browser_requests_remove_their_controls_and_detached_panels_dispose() {
        use nana_ui::runtime::{DocumentId, RuntimeDocument};
        let id = DocumentId::new(99301).unwrap();
        let mut document = RuntimeDocument::new(id);
        let sink: crate::runtime_shell::IntentSink = std::sync::Arc::new(|_| {});
        let mut view = BrowserView::mount(document.context_mut(), id, &sink).unwrap();
        let request = BrowserHostRequest {
            id: 1,
            scope: BrowserScope {
                project_id: lilia_contracts::ProjectId::new("project").unwrap(),
                task_id: lilia_contracts::TaskId::new("task").unwrap(),
                tab_id: "browser-tab".into(),
            },
            lifecycle: 1,
            page_version: 1,
            kind: BrowserHostRequestKind::Upload { multiple: false },
        };
        let target = crate::runtime_shell::ShellPaneTarget {
            window_id: crate::runtime_compat::HostedWindowId::PRIMARY,
            pane_id: "pane".into(),
            item_id: "browser-tab".into(),
        };
        let mut presentation = BrowserPresentation {
            resource: "browser-tab".into(),
            url: "about:blank".into(),
            ready: true,
            failed: false,
            human_control: false,
            busy: false,
            status: String::new(),
            requests: vec![request],
        };
        view.sync(document.context_mut(), target.clone(), &presentation)
            .unwrap();
        #[cfg(debug_assertions)]
        {
            let address_target = crate::target_ids::browser_control(
                target.window_id.0,
                &target.pane_id,
                &target.item_id,
                "address",
            );
            assert!(view.debug_input(
                &address_target,
                "http://localhost:1234/fixture",
                document.context_mut()
            ));
            let open_target = crate::target_ids::browser_control(
                target.window_id.0,
                &target.pane_id,
                &target.item_id,
                "open",
            );
            assert!(
                matches!(view.debug_click(&open_target), Some(crate::runtime_shell::ShellIntent::Browser { target: routed, action: BrowserAction::Navigate(url) }) if routed == target && url == "http://localhost:1234/fixture")
            );
            let other_window = crate::target_ids::browser_control(
                target.window_id.0 + 1,
                &target.pane_id,
                &target.item_id,
                "address",
            );
            assert!(!view.debug_input(
                &other_window,
                "http://wrong-window/",
                document.context_mut()
            ));
        }
        let row = view.request_rows[0].stable_id();
        let panel = view.requests.stable_id();
        assert!(document.context().world().contains(row));
        view.sync(document.context_mut(), target.clone(), &presentation)
            .unwrap();
        assert_eq!(view.request_rows[0].stable_id(), row);
        presentation.requests.clear();
        view.sync(document.context_mut(), target, &presentation)
            .unwrap();
        assert!(!document.context().world().contains(row));
        assert!(
            !document
                .context()
                .world()
                .node(view.root.stable_id())
                .unwrap()
                .children
                .contains(&panel)
        );
        let root = view.root.stable_id();
        let recovery = view.recovery.stable_id();
        view.dispose(document.context_mut()).unwrap();
        for node in [root, panel, recovery] {
            assert!(!document.context().world().contains(node));
        }
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn closing_selected_browser_restores_previous_tab_for_only_its_task() {
        struct Authority;
        impl lilia_agent::BrowserScopeAuthority for Authority {
            fn validate(&self, _: &BrowserScope) -> Result<(), lilia_agent::BrowserError> {
                Ok(())
            }
        }
        let mut browser = BrowserWorkbench::new(
            std::env::temp_dir().join("browser-selection-test"),
            std::sync::Arc::new(|| {}),
            std::sync::Arc::new(Authority),
        );
        let task = lilia_contracts::TaskId::new("task-a").unwrap();
        let other = lilia_contracts::TaskId::new("task-b").unwrap();
        for (tab, task_id) in [
            (
                "browser:task-a:ffffffff-ffff-4fff-8fff-ffffffffffff",
                task.clone(),
            ),
            (
                "browser:task-a:00000000-0000-4000-8000-000000000000",
                task.clone(),
            ),
            ("browser:task-b", other.clone()),
        ] {
            browser
                .open(BrowserScope {
                    project_id: lilia_contracts::ProjectId::new("project").unwrap(),
                    task_id,
                    tab_id: tab.into(),
                })
                .unwrap();
        }
        assert_eq!(
            browser.selected.borrow().get(&task).map(String::as_str),
            Some("browser:task-a:00000000-0000-4000-8000-000000000000")
        );
        browser.retain(
            &[
                "browser:task-a:ffffffff-ffff-4fff-8fff-ffffffffffff".to_string(),
                "browser:task-b".to_string(),
            ]
            .into_iter()
            .collect(),
        );
        assert_eq!(
            browser.selected.borrow().get(&task).map(String::as_str),
            Some("browser:task-a:ffffffff-ffff-4fff-8fff-ffffffffffff")
        );
        assert_eq!(
            browser.selected.borrow().get(&other).map(String::as_str),
            Some("browser:task-b")
        );
        browser.retain(&["browser:task-b".to_string()].into_iter().collect());
        assert!(!browser.selected.borrow().contains_key(&task));
        assert_eq!(
            browser.selected.borrow().get(&other).map(String::as_str),
            Some("browser:task-b")
        );
    }
}
