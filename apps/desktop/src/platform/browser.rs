use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use base64::Engine;
use lilia_agent::{BrowserCancellation, BrowserError, BrowserSessions, TaskBrowserHost};
use lilia_contracts::{BrowserHostRequest, BrowserHostRequestKind};
use lilia_contracts::{BrowserOperation, BrowserPage, BrowserRequest, BrowserScope, BrowserTarget};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use webview2_com::{
    CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR,
    CreateCoreWebView2CompositionControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler, DevToolsProtocolEventReceivedEventHandler,
    DownloadStartingEventHandler, FocusChangedEventHandler, Microsoft::Web::WebView2::Win32::*,
    NavigationCompletedEventHandler, NavigationStartingEventHandler,
    NewWindowRequestedEventHandler, SourceChangedEventHandler, StateChangedEventHandler,
};
use windows::core::{Interface, PCWSTR, PWSTR};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::DirectComposition::IDCompositionVisual;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

#[path = "browser_requests.rs"]
mod requests;

type ResponseSender = mpsc::SyncSender<Result<BrowserPage, BrowserError>>;
type HostTokens = Arc<Mutex<BTreeMap<u64, (BrowserScope, BrowserCancellation)>>>;
struct Reply {
    sender: ResponseSender,
    tab: Rc<Tab>,
    epoch: u64,
}
impl Reply {
    fn clear_operation(&self) {
        if self.epoch == self.tab.operation_epoch.get() {
            self.tab.active.set(false);
            self.tab.capturing.set(false);
            self.tab.operation_token.borrow_mut().take();
        }
    }

    fn send(
        self,
        value: Result<BrowserPage, BrowserError>,
    ) -> Result<(), mpsc::SendError<Result<BrowserPage, BrowserError>>> {
        self.clear_operation();
        self.sender.send(value)
    }
}
impl Drop for Reply {
    fn drop(&mut self) {
        self.clear_operation();
    }
}
type Completion = Box<dyn FnOnce(Result<Value, BrowserError>)>;

enum Command {
    Execute(BrowserRequest, BrowserCancellation, ResponseSender),
    Cancel(BrowserScope),
}

pub struct BrowserBridge {
    sender: mpsc::SyncSender<Command>,
    wake: Arc<dyn Fn() + Send + Sync>,
    host_tokens: HostTokens,
}

impl TaskBrowserHost for BrowserBridge {
    fn execute(
        &self,
        request: &BrowserRequest,
        cancellation: &BrowserCancellation,
    ) -> Result<BrowserPage, BrowserError> {
        if cancellation.is_cancelled() {
            return Err(BrowserError::Cancelled);
        }
        let (send, receive) = mpsc::sync_channel(1);
        self.sender
            .try_send(Command::Execute(
                request.clone(),
                cancellation.clone(),
                send,
            ))
            .map_err(|_| BrowserError::Busy)?;
        (self.wake)();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if cancellation.is_cancelled() {
                return Err(BrowserError::Cancelled);
            }
            match receive.recv_timeout(Duration::from_millis(25)) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(BrowserError::Unavailable),
                Err(_) if Instant::now() >= deadline => {
                    cancellation.cancel();
                    return Err(BrowserError::Host("Browser operation timed out".into()));
                }
                Err(_) => {}
            }
        }
    }
    fn cancel(&self, scope: &BrowserScope) {
        for (owned_scope, token) in self
            .host_tokens
            .lock()
            .expect("browser request tokens")
            .values()
        {
            if owned_scope == scope {
                token.cancel();
            }
        }
        let _ = self.sender.try_send(Command::Cancel(scope.clone()));
        (self.wake)();
    }
}

#[derive(Clone, Debug)]
pub enum BrowserHostEvent {
    Ready(BrowserScope),
    Failed(BrowserScope, String),
    Request(BrowserHostRequest),
    RequestResolved {
        request: BrowserHostRequest,
        error: Option<String>,
    },
}

#[derive(Default)]
struct HostEvents(Vec<BrowserHostEvent>);
impl HostEvents {
    fn push(&mut self, event: BrowserHostEvent) {
        if self.0.len() == 128 {
            self.0.remove(0);
        }
        self.0.push(event);
    }
    fn drain(&mut self, range: std::ops::RangeFull) -> std::vec::Drain<'_, BrowserHostEvent> {
        self.0.drain(range)
    }
}

struct Tab {
    _apartment: Rc<ComApartment>,
    environment: ICoreWebView2Environment3,
    pristine: Cell<bool>,
    document_epoch: Cell<u64>,
    navigation_epoch: Cell<u64>,
    downloads: RefCell<Vec<requests::Download>>,
    scope: BrowserScope,
    controller: ICoreWebView2Controller,
    composition: ICoreWebView2CompositionController,
    view: ICoreWebView2,
    active: Cell<bool>,
    capturing: Cell<bool>,
    visible: Cell<bool>,
    focused: Cell<bool>,
    operation_epoch: Cell<u64>,
    operation_token: RefCell<Option<BrowserCancellation>>,
    navigating: Cell<bool>,
    pending_observation: RefCell<Option<(BrowserCancellation, Option<Vec<u8>>, Reply)>>,
}

struct ComApartment(Result<(), BrowserError>);
impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.0.is_ok() {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

pub struct UiBrowserHost {
    apartment: Rc<ComApartment>,
    receiver: mpsc::Receiver<Command>,
    tabs: Rc<RefCell<BTreeMap<String, Rc<Tab>>>>,
    events: Rc<RefCell<HostEvents>>,
    sessions: Rc<RefCell<Weak<BrowserSessions>>>,
    creating: Rc<RefCell<BTreeMap<String, (BrowserScope, Rc<()>)>>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    requests: Rc<RefCell<requests::Requests>>,
    host_tokens: HostTokens,
}

impl UiBrowserHost {
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> (Self, Arc<BrowserBridge>) {
        let (sender, receiver) = mpsc::sync_channel(64);
        let host_tokens = Arc::new(Mutex::new(BTreeMap::new()));
        (
            Self {
                apartment: Rc::new(ComApartment(
                    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                        .ok()
                        .map_err(host_error),
                )),
                receiver,
                tabs: Rc::new(RefCell::new(BTreeMap::new())),
                events: Rc::new(RefCell::new(HostEvents::default())),
                sessions: Rc::new(RefCell::new(Weak::new())),
                creating: Rc::new(RefCell::new(BTreeMap::new())),
                wake: wake.clone(),
                requests: Rc::new(RefCell::new(requests::Requests::default())),
                host_tokens: host_tokens.clone(),
            },
            Arc::new(BrowserBridge {
                sender,
                wake,
                host_tokens,
            }),
        )
    }

    pub fn attach_sessions(&self, sessions: &Arc<BrowserSessions>) {
        *self.sessions.borrow_mut() = Arc::downgrade(sessions);
    }

    pub fn drain_events(&self) -> Vec<BrowserHostEvent> {
        self.events.borrow_mut().drain(..).collect()
    }

    pub fn create(
        &self,
        scope: BrowserScope,
        window: HWND,
        visual: IDCompositionVisual,
        profile_root: &Path,
    ) -> Result<(), BrowserError> {
        self.create_inner(scope, window, visual, profile_root, None)
    }

    #[cfg(debug_assertions)]
    pub(crate) fn capture_pending_count(&self) -> usize {
        self.tabs
            .borrow()
            .values()
            .filter(|tab| {
                tab.active.get()
                    && tab.capturing.get()
                    && tab
                        .operation_token
                        .borrow()
                        .as_ref()
                        .is_some_and(|token| !token.is_cancelled())
            })
            .count()
    }

    pub fn create_for_request(
        &self,
        request: &BrowserHostRequest,
        scope: BrowserScope,
        window: HWND,
        visual: IDCompositionVisual,
        profile_root: &Path,
    ) -> Result<(), BrowserError> {
        let sessions = self
            .sessions
            .borrow()
            .upgrade()
            .ok_or(BrowserError::Unavailable)?;
        self.requests.borrow().valid(request, &sessions)?;
        if !matches!(request.kind, BrowserHostRequestKind::NewWindow { .. })
            || scope.task_id != request.scope.task_id
            || scope.project_id != request.scope.project_id
        {
            return Err(BrowserError::WrongScope);
        }
        self.create_inner(
            scope,
            window,
            visual,
            profile_root,
            Some(self.tab(&request.scope)?.environment.clone()),
        )
    }

    fn create_inner(
        &self,
        scope: BrowserScope,
        window: HWND,
        visual: IDCompositionVisual,
        profile_root: &Path,
        environment: Option<ICoreWebView2Environment3>,
    ) -> Result<(), BrowserError> {
        self.apartment.0.clone()?;
        if self.tabs.borrow().contains_key(&scope.tab_id)
            || self.creating.borrow().contains_key(&scope.tab_id)
        {
            return Err(BrowserError::Busy);
        }
        let profile = project_profile_path(profile_root, scope.project_id.as_str());
        let apartment = self.apartment.clone();
        std::fs::create_dir_all(&profile).map_err(host_error)?;
        let creation = Rc::new(());
        self.creating
            .borrow_mut()
            .insert(scope.tab_id.clone(), (scope.clone(), creation.clone()));
        let profile_wide = wide(&profile.to_string_lossy());
        let tabs = self.tabs.clone();
        let events = self.events.clone();
        let sessions = self.sessions.clone();
        let creating = self.creating.clone();
        let immediate_scope = scope.clone();
        let wake = self.wake.clone();
        let requests = self.requests.clone();
        let existing_environment = environment.or_else(|| {
            self.tabs
                .borrow()
                .values()
                .find(|tab| tab.scope.project_id == scope.project_id)
                .map(|tab| tab.environment.clone())
        });
        let handler = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
            move |status, environment| {
                let environment_scope = scope.clone();
                let environment_events = events.clone();
                let environment_wake = wake.clone();
                let environment_creating = creating.clone();
                let environment_creation = creation.clone();
                let setup = (|| -> windows::core::Result<()> {
                    status?;
                    let environment = environment.ok_or_else(|| {
                        windows::core::Error::from_hresult(windows::Win32::Foundation::E_POINTER)
                    })?;
                    let environment: ICoreWebView2Environment3 = environment.cast()?;
                    let tab_environment = environment.clone();
                    let failure_scope = scope.clone();
                    let failure_events = events.clone();
                    let handler = CreateCoreWebView2CompositionControllerCompletedHandler::create(
                        Box::new(move |status, composition| {
                            let result = (|| -> Result<(), BrowserError> {
                                status.map_err(host_error)?;
                                let composition = composition.ok_or(BrowserError::Unavailable)?;
                                let controller: ICoreWebView2Controller =
                                    composition.cast().map_err(host_error)?;
                                if !creating
                                    .borrow()
                                    .get(&scope.tab_id)
                                    .is_some_and(|(_, current)| Rc::ptr_eq(current, &creation))
                                {
                                    unsafe {
                                        let _ = controller.Close();
                                    }
                                    return Err(BrowserError::Cancelled);
                                }
                                creating.borrow_mut().remove(&scope.tab_id);
                                let view =
                                    unsafe { controller.CoreWebView2() }.map_err(host_error)?;
                                unsafe {
                                    composition
                                        .SetRootVisualTarget(&visual)
                                        .map_err(host_error)?;
                                    controller.SetIsVisible(false).map_err(host_error)?;
                                    let settings = view.Settings().map_err(host_error)?;
                                    settings.SetIsWebMessageEnabled(false).map_err(host_error)?;
                                    settings
                                        .SetAreHostObjectsAllowed(false)
                                        .map_err(host_error)?;
                                }
                                let tab = Rc::new(Tab {
                                    _apartment: apartment,
                                    environment: tab_environment,
                                    pristine: Cell::new(true),
                                    document_epoch: Cell::new(1),
                                    navigation_epoch: Cell::new(1),
                                    downloads: RefCell::new(Vec::new()),
                                    scope: scope.clone(),
                                    controller,
                                    composition,
                                    view,
                                    active: Cell::new(false),
                                    capturing: Cell::new(false),
                                    visible: Cell::new(false),
                                    focused: Cell::new(false),
                                    operation_epoch: Cell::new(0),
                                    operation_token: RefCell::new(None),
                                    navigating: Cell::new(false),
                                    pending_observation: RefCell::new(None),
                                });
                                install_events(
                                    &tab,
                                    sessions.clone(),
                                    events.clone(),
                                    wake.clone(),
                                    requests.clone(),
                                )?;
                                if let Some(sessions) = sessions.borrow().upgrade() {
                                    sessions.register(scope.clone(), blank_page())?;
                                }
                                tabs.borrow_mut().insert(scope.tab_id.clone(), tab);
                                events
                                    .borrow_mut()
                                    .push(BrowserHostEvent::Ready(scope.clone()));
                                Ok(())
                            })();
                            if let Err(error) = result {
                                if creating
                                    .borrow()
                                    .get(&scope.tab_id)
                                    .is_some_and(|(_, current)| Rc::ptr_eq(current, &creation))
                                {
                                    creating.borrow_mut().remove(&scope.tab_id);
                                }
                                if error != BrowserError::Cancelled {
                                    events.borrow_mut().push(BrowserHostEvent::Failed(
                                        scope.clone(),
                                        error.to_string(),
                                    ));
                                }
                            }
                            wake();
                            Ok(())
                        }),
                    );
                    let result = unsafe {
                        environment.CreateCoreWebView2CompositionController(window, &handler)
                    };
                    if let Err(error) = &result {
                        failure_events
                            .borrow_mut()
                            .push(BrowserHostEvent::Failed(failure_scope, error.to_string()));
                    }
                    result
                })();
                if let Err(error) = setup {
                    if environment_creating
                        .borrow()
                        .get(&environment_scope.tab_id)
                        .is_some_and(|(_, current)| Rc::ptr_eq(current, &environment_creation))
                    {
                        environment_creating
                            .borrow_mut()
                            .remove(&environment_scope.tab_id);
                    }
                    environment_events
                        .borrow_mut()
                        .push(BrowserHostEvent::Failed(
                            environment_scope,
                            error.to_string(),
                        ));
                }
                environment_wake();
                Ok(())
            },
        ));
        let result = if let Some(environment) = existing_environment {
            unsafe { handler.Invoke(windows::Win32::Foundation::S_OK, &environment) }
        } else {
            unsafe {
                CreateCoreWebView2EnvironmentWithOptions(
                    PCWSTR::null(),
                    PCWSTR(profile_wide.as_ptr()),
                    None,
                    &handler,
                )
            }
        }
        .map_err(host_error);
        if result.is_err() {
            self.creating.borrow_mut().remove(&immediate_scope.tab_id);
        }
        result
    }

    pub fn poll(&self) {
        self.poll_downloads();
        if let Some(sessions) = self.sessions.borrow().upgrade() {
            let expired = self.requests.borrow_mut().sweep(&sessions);
            for pending in &expired {
                self.events
                    .borrow_mut()
                    .push(BrowserHostEvent::RequestResolved {
                        request: pending.ticket.clone(),
                        error: Some("Browser request is no longer available".into()),
                    });
            }
            drop(expired);
        }
        for _ in 0..64 {
            let Ok(command) = self.receiver.try_recv() else {
                break;
            };
            match command {
                Command::Execute(request, token, reply) => {
                    let validation = self
                        .sessions
                        .borrow()
                        .upgrade()
                        .ok_or(BrowserError::Unavailable)
                        .and_then(|sessions| sessions.validate_scope(&request.scope));
                    if let Err(error) = validation {
                        token.cancel();
                        let _ = reply.send(Err(error));
                        continue;
                    }
                    let tab = self.tabs.borrow().get(&request.scope.tab_id).cloned();
                    match tab {
                        Some(tab) if tab.scope == request.scope => {
                            tab.active.set(true);
                            tab.capturing
                                .set(matches!(request.operation, BrowserOperation::Screenshot));
                            tab.operation_epoch.set(tab.operation_epoch.get() + 1);
                            *tab.operation_token.borrow_mut() = Some(token.clone());
                            let reply = Reply {
                                sender: reply,
                                tab: tab.clone(),
                                epoch: tab.operation_epoch.get(),
                            };
                            run_operation(tab, request.operation, token, reply)
                        }
                        _ => {
                            let _ = reply.send(Err(BrowserError::UnknownTab));
                        }
                    }
                }
                Command::Cancel(scope) => {
                    if let Some(tab) = self
                        .tabs
                        .borrow()
                        .get(&scope.tab_id)
                        .filter(|tab| tab.scope == scope)
                    {
                        if tab
                            .operation_token
                            .borrow()
                            .as_ref()
                            .is_some_and(BrowserCancellation::is_cancelled)
                        {
                            unsafe {
                                let _ = tab.view.Stop();
                            }
                            if let Some((_, _, reply)) = tab.pending_observation.borrow_mut().take()
                            {
                                let _ = reply.send(Err(BrowserError::Cancelled));
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn set_bounds(
        &self,
        scope: &BrowserScope,
        bounds: RECT,
        visible: bool,
    ) -> Result<(), BrowserError> {
        let tab = self.tab(scope)?;
        unsafe { tab.controller.SetBounds(bounds) }.map_err(host_error)?;
        self.set_visible(scope, visible)
    }

    pub fn focus(&self, scope: &BrowserScope) -> Result<(), BrowserError> {
        if !self.tab(scope)?.visible.get() {
            return Err(BrowserError::Unavailable);
        }
        let tab = self.takeover_tab(scope)?;
        unsafe {
            tab.controller
                .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
        }
        .map_err(host_error)
    }

    pub fn set_visible(&self, scope: &BrowserScope, visible: bool) -> Result<(), BrowserError> {
        let tab = self.tab(scope)?;
        unsafe { tab.controller.SetIsVisible(visible) }.map_err(host_error)?;
        if tab.visible.replace(visible) && !visible && tab.focused.get() {
            self.blur(scope)?;
        }
        Ok(())
    }

    pub fn set_pointer_capture(
        &self,
        scope: &BrowserScope,
        capture: bool,
    ) -> Result<(), BrowserError> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture};
        let tab = self.tab(scope)?;
        let mut parent = HWND::default();
        unsafe {
            tab.controller
                .ParentWindow(&mut parent)
                .map_err(host_error)?;
            if capture {
                if !tab.visible.get() {
                    return Err(BrowserError::Unavailable);
                }
                SetCapture(parent);
            } else if GetCapture() == parent {
                ReleaseCapture().map_err(host_error)?;
            }
        }
        Ok(())
    }

    pub fn blur(&self, scope: &BrowserScope) -> Result<(), BrowserError> {
        let tab = self.tab(scope)?;
        if !tab.focused.get() {
            return Ok(());
        }
        let mut parent = HWND::default();
        unsafe {
            tab.controller
                .ParentWindow(&mut parent)
                .map_err(host_error)?;
            windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(parent))
                .map_err(host_error)?;
        }
        tab.focused.set(false);
        Ok(())
    }

    pub fn reparent(
        &self,
        scope: &BrowserScope,
        window: HWND,
        visual: IDCompositionVisual,
    ) -> Result<(), BrowserError> {
        let tab = self.tab(scope)?;
        let mut previous = HWND::default();
        unsafe {
            let previous_visual = tab.composition.RootVisualTarget().map_err(host_error)?;
            tab.controller
                .ParentWindow(&mut previous)
                .map_err(host_error)?;
            tab.controller.SetParentWindow(window).map_err(host_error)?;
            if let Err(error) = tab.composition.SetRootVisualTarget(&visual) {
                let visual_restore = tab.composition.SetRootVisualTarget(&previous_visual);
                if let Err(restore_error) = tab.controller.SetParentWindow(previous) {
                    return Err(BrowserError::Host(format!(
                        "Reparent failed: {error}; parent restore failed: {restore_error}"
                    )));
                }
                visual_restore.map_err(host_error)?;
                return Err(host_error(error));
            }
        }
        Ok(())
    }

    pub fn navigate(&self, scope: &BrowserScope, url: &str) -> Result<(), BrowserError> {
        if !(url.starts_with("https://") || url.starts_with("http://") || url == "about:blank") {
            return Err(BrowserError::Host("Unsupported browser address".into()));
        }
        let tab = self.takeover_tab(scope)?;
        tab.pristine.set(false);
        unsafe { tab.view.Navigate(PCWSTR(wide(url).as_ptr())) }.map_err(host_error)
    }

    pub fn back(&self, scope: &BrowserScope) -> Result<(), BrowserError> {
        unsafe { self.takeover_tab(scope)?.view.GoBack() }.map_err(host_error)
    }

    pub fn forward(&self, scope: &BrowserScope) -> Result<(), BrowserError> {
        unsafe { self.takeover_tab(scope)?.view.GoForward() }.map_err(host_error)
    }

    pub fn reload(&self, scope: &BrowserScope) -> Result<(), BrowserError> {
        unsafe { self.takeover_tab(scope)?.view.Reload() }.map_err(host_error)
    }

    fn takeover_tab(&self, scope: &BrowserScope) -> Result<Rc<Tab>, BrowserError> {
        let tab = self.tab(scope)?;
        if let Some(sessions) = self.sessions.borrow().upgrade() {
            sessions.validate_scope(scope)?;
            if sessions.state(scope)?.control == lilia_contracts::BrowserControl::Agent {
                sessions.takeover(scope)?;
                unsafe {
                    let _ = tab.view.Stop();
                }
                tab.operation_token.borrow_mut().take();
                tab.operation_epoch.set(tab.operation_epoch.get() + 1);
                tab.active.set(false);
                if let Some((_, _, reply)) = tab.pending_observation.borrow_mut().take() {
                    let _ = reply.send(Err(BrowserError::Cancelled));
                }
            }
        }
        Ok(tab)
    }

    pub fn set_zoom(&self, scope: &BrowserScope, factor: f64) -> Result<(), BrowserError> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(BrowserError::Host("Invalid browser zoom".into()));
        }
        unsafe { self.tab(scope)?.controller.SetZoomFactor(factor) }.map_err(host_error)
    }

    pub fn set_rasterization_scale(
        &self,
        scope: &BrowserScope,
        factor: f64,
    ) -> Result<(), BrowserError> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(BrowserError::Host("Invalid browser display scale".into()));
        }
        let controller: ICoreWebView2Controller3 =
            self.tab(scope)?.controller.cast().map_err(host_error)?;
        unsafe {
            controller
                .SetShouldDetectMonitorScaleChanges(false)
                .map_err(host_error)?;
            controller.SetRasterizationScale(factor).map_err(host_error)
        }
    }

    pub fn mouse_input(
        &self,
        scope: &BrowserScope,
        kind: COREWEBVIEW2_MOUSE_EVENT_KIND,
        keys: COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS,
        data: u32,
        point: POINT,
    ) -> Result<(), BrowserError> {
        if !self.tab(scope)?.visible.get() {
            return Err(BrowserError::Unavailable);
        }
        let tab = if !matches!(
            kind,
            COREWEBVIEW2_MOUSE_EVENT_KIND_MOVE | COREWEBVIEW2_MOUSE_EVENT_KIND_LEAVE
        ) {
            self.takeover_tab(scope)?
        } else {
            self.tab(scope)?
        };
        unsafe { tab.composition.SendMouseInput(kind, keys, data, point) }.map_err(host_error)
    }

    pub fn close(&self, scope: &BrowserScope) -> Result<(), BrowserError> {
        self.cancel_downloads(scope);
        let cancelled = self.requests.borrow_mut().cancel_scope(scope);
        for pending in &cancelled {
            self.events
                .borrow_mut()
                .push(BrowserHostEvent::RequestResolved {
                    request: pending.ticket.clone(),
                    error: Some("Browser request cancelled".into()),
                });
        }
        drop(cancelled);
        if self
            .creating
            .borrow()
            .get(&scope.tab_id)
            .is_some_and(|(pending, _)| pending == scope)
        {
            self.creating.borrow_mut().remove(&scope.tab_id);
            return Ok(());
        }
        let tab = self.tab(scope)?;
        if let Some(sessions) = self.sessions.borrow().upgrade() {
            sessions.close(scope)?;
        }
        self.tabs.borrow_mut().remove(&scope.tab_id);
        if let Some((_, _, reply)) = tab.pending_observation.borrow_mut().take() {
            let _ = reply.send(Err(BrowserError::Cancelled));
        }
        unsafe { tab.controller.Close() }.map_err(host_error)
    }

    fn tab(&self, scope: &BrowserScope) -> Result<Rc<Tab>, BrowserError> {
        self.tabs
            .borrow()
            .get(&scope.tab_id)
            .filter(|tab| tab.scope == *scope)
            .cloned()
            .ok_or(BrowserError::UnknownTab)
    }
}

fn project_profile_path(root: &Path, project: &str) -> PathBuf {
    root.join(digest_hex(project.as_bytes()))
}

fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Drop for UiBrowserHost {
    fn drop(&mut self) {
        let mut tokens = self.host_tokens.lock().expect("browser request tokens");
        for (_, token) in tokens.values() {
            token.cancel();
        }
        tokens.clear();
        drop(tokens);
        let cancelled = self.requests.borrow_mut().clear();
        drop(cancelled);
        self.creating.borrow_mut().clear();
        let tabs = std::mem::take(&mut *self.tabs.borrow_mut());
        for tab in tabs.into_values() {
            for download in tab.downloads.borrow_mut().drain(..) {
                unsafe {
                    let _ = download.operation.Cancel();
                }
            }
            if let Some(sessions) = self.sessions.borrow().upgrade() {
                let _ = sessions.close(&tab.scope);
            }
            if let Some(token) = tab.operation_token.borrow().as_ref() {
                token.cancel();
            }
            if let Some((_, _, reply)) = tab.pending_observation.borrow_mut().take() {
                let _ = reply.send(Err(BrowserError::Cancelled));
            }
            unsafe {
                let _ = tab.controller.Close();
            }
        }
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn host_error(error: impl std::fmt::Display) -> BrowserError {
    BrowserError::Host(error.to_string())
}
fn blank_page() -> BrowserPage {
    BrowserPage {
        url: "about:blank".into(),
        title: String::new(),
        targets: vec![],
        screenshot_artifact: None,
        screenshot_bytes: None,
    }
}

fn current_page(view: &ICoreWebView2) -> Result<BrowserPage, BrowserError> {
    let mut url = PWSTR::null();
    let mut title = PWSTR::null();
    unsafe {
        view.Source(&mut url).map_err(host_error)?;
    }
    let url = CoTaskMemPWSTR::from(url).to_string();
    unsafe {
        view.DocumentTitle(&mut title).map_err(host_error)?;
    }
    Ok(BrowserPage {
        url,
        title: CoTaskMemPWSTR::from(title).to_string(),
        targets: vec![],
        screenshot_artifact: None,
        screenshot_bytes: None,
    })
}

fn cdp(
    view: &ICoreWebView2,
    method: &str,
    parameters: Value,
    token: &BrowserCancellation,
    completed: Completion,
) {
    if token.is_cancelled() {
        completed(Err(BrowserError::Cancelled));
        return;
    }
    let completed = Rc::new(RefCell::new(Some(completed)));
    let callback_completed = completed.clone();
    let callback_token = token.clone();
    let handler =
        CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |status, result| {
            let result = if callback_token.is_cancelled() {
                Err(BrowserError::Cancelled)
            } else {
                status
                    .map_err(host_error)
                    .map(|_| result)
                    .and_then(|text| serde_json::from_str::<Value>(&text).map_err(host_error))
                    .and_then(|value| {
                        if value.get("error").is_some() {
                            Err(BrowserError::Host(
                                "Browser protocol rejected the operation".into(),
                            ))
                        } else {
                            Ok(value)
                        }
                    })
            };
            if let Some(completed) = callback_completed.borrow_mut().take() {
                completed(result);
            }
            Ok(())
        }));
    let method = wide(method);
    let parameters = wide(&parameters.to_string());
    let result = unsafe {
        view.CallDevToolsProtocolMethod(
            PCWSTR(method.as_ptr()),
            PCWSTR(parameters.as_ptr()),
            &handler,
        )
    };
    if let Err(error) = result {
        if let Some(completed) = completed.borrow_mut().take() {
            completed(Err(host_error(error)));
        }
    }
}

fn observe(tab: Rc<Tab>, token: BrowserCancellation, screenshot: Option<Vec<u8>>, reply: Reply) {
    if token.is_cancelled() {
        let _ = reply.send(Err(BrowserError::Cancelled));
        return;
    }
    if tab.navigating.get() {
        *tab.pending_observation.borrow_mut() = Some((token, screenshot, reply));
        return;
    }
    let view = tab.view.clone();
    let next_token = token.clone();
    cdp(
        &view,
        "DOM.getDocument",
        json!({"depth":-1,"pierce":true}),
        &token,
        Box::new(move |result| {
            if let Err(error) = result {
                let _ = reply.send(Err(error));
                return;
            }
            observe_accessibility(tab, next_token, screenshot, reply);
        }),
    );
}

fn observe_accessibility(
    tab: Rc<Tab>,
    token: BrowserCancellation,
    screenshot: Option<Vec<u8>>,
    reply: Reply,
) {
    let view = tab.view.clone();
    cdp(
        &view,
        "Accessibility.getFullAXTree",
        json!({}),
        &token,
        Box::new(move |result| {
            let result = result.and_then(|value| {
                let targets = value["nodes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|node| node["ignored"].as_bool() != Some(true))
                    .filter_map(|node| {
                        let id = node["backendDOMNodeId"].as_u64()?;
                        let role = node["role"]["value"].as_str()?.to_owned();
                        let name = node["name"]["value"]
                            .as_str()
                            .unwrap_or_default()
                            .chars()
                            .take(500)
                            .collect();
                        Some(BrowserTarget {
                            id: id.to_string(),
                            role,
                            name,
                        })
                    })
                    .take(1000)
                    .collect();
                let mut page = current_page(&tab.view)?;
                page.targets = targets;
                page.screenshot_artifact = None;
                page.screenshot_bytes = screenshot;
                Ok(page)
            });
            let _ = reply.send(result);
        }),
    );
}

fn run_operation(
    tab: Rc<Tab>,
    operation: BrowserOperation,
    token: BrowserCancellation,
    reply: Reply,
) {
    if matches!(
        operation,
        BrowserOperation::Navigate { .. }
            | BrowserOperation::Back
            | BrowserOperation::Forward
            | BrowserOperation::Reload
    ) {
        tab.pristine.set(false);
    }
    if token.is_cancelled() {
        let _ = reply.send(Err(BrowserError::Cancelled));
        return;
    }
    if matches!(operation, BrowserOperation::Observe) {
        observe(tab, token, None, reply);
        return;
    }
    if let BrowserOperation::Click { target } | BrowserOperation::Type { target, .. } = &operation {
        let Ok(node) = target.parse::<u64>() else {
            let _ = reply.send(Err(BrowserError::StalePage));
            return;
        };
        let view = tab.view.clone();
        let after_token = token.clone();
        if let BrowserOperation::Type { text, .. } = operation {
            cdp(
                &view,
                "DOM.focus",
                json!({"backendNodeId":node}),
                &token,
                Box::new(move |result| {
                    if let Err(error) = result {
                        let _ = reply.send(Err(error));
                        return;
                    }
                    let view = tab.view.clone();
                    let final_token = after_token.clone();
                    cdp(
                        &view,
                        "Input.insertText",
                        json!({"text":text}),
                        &after_token,
                        Box::new(move |result| finish(tab, final_token, reply, result)),
                    );
                }),
            );
        } else {
            cdp(
                &view,
                "DOM.getBoxModel",
                json!({"backendNodeId":node}),
                &token,
                Box::new(move |result| {
                    let center = result.and_then(|value| {
                        let quad = value["model"]["content"]
                            .as_array()
                            .ok_or(BrowserError::StalePage)?;
                        if quad.len() != 8 {
                            return Err(BrowserError::StalePage);
                        }
                        Ok((
                            (quad[0].as_f64().unwrap_or(0.) + quad[4].as_f64().unwrap_or(0.)) / 2.,
                            (quad[1].as_f64().unwrap_or(0.) + quad[5].as_f64().unwrap_or(0.)) / 2.,
                        ))
                    });
                    let (x, y) = match center {
                        Ok(center) => center,
                        Err(error) => {
                            let _ = reply.send(Err(error));
                            return;
                        }
                    };
                    let view = tab.view.clone();
                    let release_token = after_token.clone();
                    cdp(
                        &view,
                        "Input.dispatchMouseEvent",
                        json!({"type":"mousePressed","x":x,"y":y,"button":"left","clickCount":1}),
                        &after_token,
                        Box::new(move |result| {
                            if let Err(error) = result {
                                let _ = reply.send(Err(error));
                                return;
                            }
                            let view = tab.view.clone();
                            let final_token = release_token.clone();
                            cdp(
                                &view,
                                "Input.dispatchMouseEvent",
                                json!({"type":"mouseReleased","x":x,"y":y,"button":"left","clickCount":1}),
                                &release_token,
                                Box::new(move |result| finish(tab, final_token, reply, result)),
                            );
                        }),
                    );
                }),
            );
        }
        return;
    }
    let screenshot = matches!(operation, BrowserOperation::Screenshot);
    let (method, parameters) = match operation {
        BrowserOperation::Navigate { url }
            if url.starts_with("https://")
                || url.starts_with("http://")
                || url == "about:blank" =>
        {
            ("Page.navigate", json!({"url":url}))
        }
        BrowserOperation::Reload => ("Page.reload", json!({})),
        BrowserOperation::Scroll { x, y } => (
            "Input.dispatchMouseEvent",
            json!({"type":"mouseWheel","x":1,"y":1,"deltaX":x,"deltaY":y}),
        ),
        BrowserOperation::Screenshot => ("Page.captureScreenshot", json!({"format":"png"})),
        BrowserOperation::Back | BrowserOperation::Forward => {
            let back = matches!(operation, BrowserOperation::Back);
            let view = tab.view.clone();
            let next_token = token.clone();
            cdp(
                &view,
                "Page.getNavigationHistory",
                json!({}),
                &token,
                Box::new(move |result| {
                    let entry = result.and_then(|value| {
                        let current = value["currentIndex"]
                            .as_i64()
                            .ok_or(BrowserError::StalePage)?;
                        let index = current + if back { -1 } else { 1 };
                        if index < 0 {
                            return Err(BrowserError::Host("No navigation history".into()));
                        }
                        value["entries"][index as usize]["id"]
                            .as_i64()
                            .ok_or(BrowserError::StalePage)
                    });
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(error) => {
                            let _ = reply.send(Err(error));
                            return;
                        }
                    };
                    let view = tab.view.clone();
                    let final_token = next_token.clone();
                    cdp(
                        &view,
                        "Page.navigateToHistoryEntry",
                        json!({"entryId":entry}),
                        &next_token,
                        Box::new(move |result| finish(tab, final_token, reply, result)),
                    );
                }),
            );
            return;
        }
        _ => {
            let _ = reply.send(Err(BrowserError::Host(
                "Unsupported browser operation".into(),
            )));
            return;
        }
    };
    let view = tab.view.clone();
    let final_token = token.clone();
    cdp(
        &view,
        method,
        parameters,
        &token,
        Box::new(move |result| {
            if screenshot {
                let artifact = result.and_then(|value| {
                    let data = value["data"]
                        .as_str()
                        .ok_or_else(|| BrowserError::Host("Screenshot is unavailable".into()))?;
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(data)
                        .map_err(host_error)?;
                    Ok(bytes)
                });
                match artifact {
                    Ok(bytes) => observe(tab, final_token, Some(bytes), reply),
                    Err(error) => {
                        let _ = reply.send(Err(error));
                    }
                }
            } else {
                finish(tab, final_token, reply, result);
            }
        }),
    );
}

fn finish(
    tab: Rc<Tab>,
    token: BrowserCancellation,
    reply: Reply,
    result: Result<Value, BrowserError>,
) {
    match result {
        Ok(_) => observe(tab, token, None, reply),
        Err(error) => {
            let _ = reply.send(Err(error));
        }
    }
}

fn install_events(
    tab: &Rc<Tab>,
    sessions: Rc<RefCell<Weak<BrowserSessions>>>,
    events: Rc<RefCell<HostEvents>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    requests: Rc<RefCell<requests::Requests>>,
) -> Result<(), BrowserError> {
    let focused_tab = Rc::downgrade(tab);
    let got_focus = FocusChangedEventHandler::create(Box::new(move |_, _| {
        if let Some(tab) = focused_tab.upgrade() {
            tab.focused.set(true);
        }
        Ok(())
    }));
    let focused_tab = Rc::downgrade(tab);
    let lost_focus = FocusChangedEventHandler::create(Box::new(move |_, _| {
        if let Some(tab) = focused_tab.upgrade() {
            tab.focused.set(false);
        }
        Ok(())
    }));
    let mut focus_token = 0;
    unsafe {
        tab.controller
            .add_GotFocus(&got_focus, &mut focus_token)
            .map_err(host_error)?;
        tab.controller
            .add_LostFocus(&lost_focus, &mut focus_token)
            .map_err(host_error)?;
    }
    let scope = tab.scope.clone();
    let navigation_tab = Rc::downgrade(tab);
    let navigation_sessions = sessions.clone();
    let navigation_wake = wake.clone();
    let navigation = NavigationStartingEventHandler::create(Box::new(move |_, args| {
        let Some(tab) = navigation_tab.upgrade() else {
            return Ok(());
        };
        tab.navigating.set(true);
        let navigation_url = if let Some(args) = args {
            let mut uri = PWSTR::null();
            unsafe {
                args.Uri(&mut uri)?;
            }
            let url = CoTaskMemPWSTR::from(uri).to_string();
            if url != "about:blank" {
                tab.pristine.set(false);
            }
            Some(url)
        } else {
            None
        };
        tab.document_epoch.set(tab.document_epoch.get() + 1);
        tab.navigation_epoch.set(tab.navigation_epoch.get() + 1);
        if tab.active.get() && tab.capturing.get() {
            if let Some(token) = tab.operation_token.borrow().as_ref() {
                token.cancel();
            }
        }
        if !tab.active.get() {
            if let Some(sessions) = navigation_sessions.borrow().upgrade() {
                if let Some(mut page) = navigation_url
                    .map(|url| BrowserPage {
                        url,
                        title: String::new(),
                        targets: Vec::new(),
                        screenshot_artifact: None,
                        screenshot_bytes: None,
                    })
                    .or_else(|| current_page(&tab.view).ok())
                {
                    page.targets.clear();
                    let _ = sessions.page_changed(&scope, page);
                }
            }
        }
        navigation_wake();
        Ok(())
    }));
    let source_tab = Rc::downgrade(tab);
    let source_sessions = sessions.clone();
    let source_wake = wake.clone();
    let source_changed = SourceChangedEventHandler::create(Box::new(move |_, _| {
        if let Some(tab) = source_tab.upgrade() {
            tab.document_epoch.set(tab.document_epoch.get() + 1);
            tab.navigation_epoch.set(tab.navigation_epoch.get() + 1);
            if tab.active.get() && tab.capturing.get() {
                if let Some(token) = tab.operation_token.borrow().as_ref() {
                    token.cancel();
                }
            }
            if !tab.active.get() {
                if let Some(sessions) = source_sessions.borrow().upgrade() {
                    if let Ok(page) = current_page(&tab.view) {
                        let _ = sessions.page_changed(&tab.scope, page);
                    }
                }
            }
        }
        source_wake();
        Ok(())
    }));
    let completed_tab = Rc::downgrade(tab);
    let completed_wake = wake.clone();
    let completed_sessions = sessions.clone();
    let navigation_completed = NavigationCompletedEventHandler::create(Box::new(move |_, _| {
        if let Some(tab) = completed_tab.upgrade() {
            tab.navigating.set(false);
            let pending = tab.pending_observation.borrow_mut().take();
            if let Some((token, screenshot, reply)) = pending {
                observe(tab, token, screenshot, reply);
            } else if !tab.active.get() {
                if let Some(sessions) = completed_sessions.borrow().upgrade() {
                    if let Ok(page) = current_page(&tab.view) {
                        let _ = sessions.page_changed(&tab.scope, page);
                    }
                }
            }
        }
        completed_wake();
        Ok(())
    }));
    let new_tab = Rc::downgrade(tab);
    let new_sessions = sessions.clone();
    let new_requests = requests.clone();
    let new_events = events.clone();
    let new_wake = wake.clone();
    let new_window = NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
        if let Some(args) = args {
            unsafe {
                args.SetHandled(true)?;
                let mut uri = PWSTR::null();
                args.Uri(&mut uri)?;
                let url = CoTaskMemPWSTR::from(uri).to_string();
                let deferral = args.GetDeferral()?;
                if let (Some(tab), Some(sessions)) =
                    (new_tab.upgrade(), new_sessions.borrow().upgrade())
                {
                    let result = new_requests.borrow_mut().insert(
                        tab,
                        &sessions,
                        BrowserHostRequestKind::NewWindow { url },
                        requests::Payload::NewWindow(args.clone()),
                        Some(deferral),
                    );
                    if let Ok(ticket) = result {
                        new_events
                            .borrow_mut()
                            .push(BrowserHostEvent::Request(ticket));
                    }
                } else {
                    deferral.Complete()?;
                }
            }
        }
        new_wake();
        Ok(())
    }));
    let download_tab = Rc::downgrade(tab);
    let download_sessions = sessions.clone();
    let download_requests = requests.clone();
    let download_events = events.clone();
    let download_wake = wake.clone();
    let download = DownloadStartingEventHandler::create(Box::new(move |_, args| {
        if let Some(args) = args {
            unsafe {
                args.SetCancel(true)?;
                let mut path = PWSTR::null();
                args.ResultFilePath(&mut path)?;
                let path = CoTaskMemPWSTR::from(path).to_string();
                let suggested_filename = Path::new(&path)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "download".into());
                let deferral = args.GetDeferral()?;
                if let (Some(tab), Some(sessions)) =
                    (download_tab.upgrade(), download_sessions.borrow().upgrade())
                {
                    let result = download_requests.borrow_mut().insert(
                        tab,
                        &sessions,
                        BrowserHostRequestKind::Download { suggested_filename },
                        requests::Payload::Download(args.clone()),
                        Some(deferral),
                    );
                    if let Ok(ticket) = result {
                        download_events
                            .borrow_mut()
                            .push(BrowserHostEvent::Request(ticket));
                    }
                } else {
                    deferral.Complete()?;
                }
            }
        }
        download_wake();
        Ok(())
    }));
    let mut token = 0;
    unsafe {
        tab.view
            .add_NavigationStarting(&navigation, &mut token)
            .map_err(host_error)?;
        tab.view
            .add_SourceChanged(&source_changed, &mut token)
            .map_err(host_error)?;
        tab.view
            .add_NavigationCompleted(&navigation_completed, &mut token)
            .map_err(host_error)?;
        tab.view
            .add_NewWindowRequested(&new_window, &mut token)
            .map_err(host_error)?;
        if let Ok(view) = tab.view.cast::<ICoreWebView2_4>() {
            view.add_DownloadStarting(&download, &mut token)
                .map_err(host_error)?;
        }
    }
    for name in [
        "DOM.documentUpdated",
        "DOM.attributeModified",
        "DOM.childNodeInserted",
        "DOM.childNodeRemoved",
        "DOM.characterDataModified",
    ] {
        let weak_tab = Rc::downgrade(tab);
        let sessions = sessions.clone();
        let event_wake = wake.clone();
        let receiver = unsafe {
            tab.view
                .GetDevToolsProtocolEventReceiver(PCWSTR(wide(name).as_ptr()))
        }
        .map_err(host_error)?;
        let handler = DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, _| {
            if let Some(tab) = weak_tab.upgrade() {
                tab.document_epoch.set(tab.document_epoch.get() + 1);
                if tab.active.get() && tab.capturing.get() {
                    if let Some(token) = tab.operation_token.borrow().as_ref() {
                        token.cancel();
                    }
                }
                if !tab.active.get() {
                    if let Some(sessions) = sessions.borrow().upgrade() {
                        if let Ok(state) = sessions.state(&tab.scope) {
                            let mut page = state.page;
                            page.targets.clear();
                            let _ = sessions.page_changed(&tab.scope, page);
                        }
                    }
                }
            }
            event_wake();
            Ok(())
        }));
        unsafe { receiver.add_DevToolsProtocolEventReceived(&handler, &mut token) }
            .map_err(host_error)?;
    }
    let upload_tab = Rc::downgrade(tab);
    let upload_sessions = sessions.clone();
    let upload_requests = requests.clone();
    let upload_wake = wake.clone();
    let receiver = unsafe {
        tab.view
            .GetDevToolsProtocolEventReceiver(PCWSTR(wide("Page.fileChooserOpened").as_ptr()))
    }
    .map_err(host_error)?;
    let handler = DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, args| {
        if let (Some(tab), Some(sessions), Some(args)) = (
            upload_tab.upgrade(),
            upload_sessions.borrow().upgrade(),
            args,
        ) {
            let mut data = PWSTR::null();
            unsafe {
                args.ParameterObjectAsJson(&mut data)?;
            }
            let data = CoTaskMemPWSTR::from(data).to_string();
            if let Ok(data) = serde_json::from_str::<Value>(&data) {
                if let Some(node) = data["backendNodeId"].as_u64() {
                    let multiple = data["mode"].as_str() == Some("selectMultiple");
                    let result = upload_requests.borrow_mut().insert(
                        tab,
                        &sessions,
                        BrowserHostRequestKind::Upload { multiple },
                        requests::Payload::Upload { node, multiple },
                        None,
                    );
                    if let Ok(ticket) = result {
                        events.borrow_mut().push(BrowserHostEvent::Request(ticket));
                    }
                }
            }
        }
        upload_wake();
        Ok(())
    }));
    unsafe { receiver.add_DevToolsProtocolEventReceived(&handler, &mut token) }
        .map_err(host_error)?;
    let token = BrowserCancellation::default();
    cdp(
        &tab.view,
        "Page.setInterceptFileChooserDialog",
        json!({"enabled":true}),
        &token,
        Box::new(|_| {}),
    );
    cdp(&tab.view, "DOM.enable", json!({}), &token, Box::new(|_| {}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_profiles_are_isolated_and_untrusted_ids_cannot_escape_profile_root() {
        let root = Path::new("C:/Lilia/browser-profiles");
        let first = project_profile_path(root, "project-a");
        assert_eq!(first, project_profile_path(root, "project-a"));
        assert_ne!(first, project_profile_path(root, "project-b"));
        assert_eq!(
            project_profile_path(root, "../../outside").parent(),
            Some(root)
        );
    }
}
