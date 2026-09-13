use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use lilia_contracts::{BrowserControl, BrowserPage, BrowserRequest, BrowserScope, BrowserState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserError {
    Unavailable,
    UnknownTab,
    WrongScope,
    StalePage,
    StaleLifecycle,
    HumanControl,
    Busy,
    Cancelled,
    PermissionDenied,
    Host(String),
}

impl std::fmt::Display for BrowserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for BrowserError {}

#[derive(Clone, Default)]
pub struct BrowserCancellation(Arc<AtomicBool>, Option<Arc<BrowserAuthorityGuard>>);
struct BrowserAuthorityGuard {
    authority: Arc<dyn BrowserScopeAuthority>,
    scope: BrowserScope,
}
impl BrowserCancellation {
    pub fn is_cancelled(&self) -> bool {
        if self.0.load(Ordering::Acquire) {
            return true;
        }
        if self
            .1
            .as_ref()
            .is_some_and(|guard| guard.authority.validate(&guard.scope).is_err())
        {
            self.cancel();
            return true;
        }
        false
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}

pub trait TaskBrowserHost: Send + Sync {
    fn execute(
        &self,
        request: &BrowserRequest,
        cancellation: &BrowserCancellation,
    ) -> Result<BrowserPage, BrowserError>;
    fn cancel(&self, scope: &BrowserScope);
}

pub trait BrowserScopeAuthority: Send + Sync {
    fn validate(&self, scope: &BrowserScope) -> Result<(), BrowserError>;
}

/// Application-owned persistence hook for successful Agent screenshots.
/// The browser domain never knows Product storage details or exposes paths.
pub trait BrowserArtifactSink: Send + Sync {
    fn record_screenshot(
        &self,
        scope: &BrowserScope,
        session: &str,
        turn: &str,
        bytes: &[u8],
        cancellation: &BrowserCancellation,
    ) -> Result<String, BrowserError>;
}

struct Tab {
    state: BrowserState,
    active: Option<BrowserCancellation>,
}

pub struct BrowserSessions {
    host: Arc<dyn TaskBrowserHost>,
    authority: Option<Arc<dyn BrowserScopeAuthority>>,
    pub(crate) private_inputs: Arc<Mutex<crate::browser_tool::BrowserPrivateInputs>>,
    tabs: Mutex<BTreeMap<String, Tab>>,
    next_lifecycle: std::sync::atomic::AtomicU64,
    agent_scopes: Mutex<BTreeMap<String, BrowserScope>>,
    selected_scopes: Mutex<BTreeMap<lilia_contracts::TaskId, BrowserScope>>,
    cancelled_turns: Mutex<BTreeSet<(String, String)>>,
    artifact_sink: Option<Arc<dyn BrowserArtifactSink>>,
    /// Serializes cancellation/invalidation with the short artifact commit
    /// critical section so a user cancel cannot interleave with persistence.
    artifact_commit: Mutex<()>,
}

impl BrowserSessions {
    fn release_active(&self, scope: &BrowserScope, token: &BrowserCancellation) {
        if let Ok(mut tabs) = self.tabs.lock() {
            if let Some(tab) = tabs.get_mut(&scope.tab_id) {
                if tab
                    .active
                    .as_ref()
                    .is_some_and(|active| Arc::ptr_eq(&active.0, &token.0))
                {
                    tab.active = None;
                }
            }
        }
    }

    pub fn new(host: Arc<dyn TaskBrowserHost>) -> Self {
        Self {
            host,
            authority: None,
            private_inputs: Arc::new(Mutex::new(Default::default())),
            tabs: Mutex::new(BTreeMap::new()),
            next_lifecycle: std::sync::atomic::AtomicU64::new(1),
            agent_scopes: Mutex::new(BTreeMap::new()),
            selected_scopes: Mutex::new(BTreeMap::new()),
            cancelled_turns: Mutex::new(BTreeSet::new()),
            artifact_sink: None,
            artifact_commit: Mutex::new(()),
        }
    }

    pub fn with_authority(
        host: Arc<dyn TaskBrowserHost>,
        authority: Arc<dyn BrowserScopeAuthority>,
    ) -> Self {
        let mut sessions = Self::new(host);
        sessions.authority = Some(authority);
        sessions
    }

    pub fn with_authority_and_artifact_sink(
        host: Arc<dyn TaskBrowserHost>,
        authority: Arc<dyn BrowserScopeAuthority>,
        artifact_sink: Arc<dyn BrowserArtifactSink>,
    ) -> Self {
        let mut sessions = Self::with_authority(host, authority);
        sessions.artifact_sink = Some(artifact_sink);
        sessions
    }

    pub fn validate_scope(&self, scope: &BrowserScope) -> Result<(), BrowserError> {
        let result = self
            .authority
            .as_ref()
            .ok_or(BrowserError::Unavailable)
            .and_then(|authority| authority.validate(scope));
        if result.is_err() {
            let _ = self.close(scope);
        }
        result
    }

    #[cfg(test)]
    pub(crate) fn for_test(host: Arc<dyn TaskBrowserHost>) -> Self {
        struct AllowScope;
        impl BrowserScopeAuthority for AllowScope {
            fn validate(&self, _: &BrowserScope) -> Result<(), BrowserError> {
                Ok(())
            }
        }
        Self::with_authority(host, Arc::new(AllowScope))
    }

    pub fn select_agent_scope(&self, scope: &BrowserScope) -> Result<(), BrowserError> {
        self.validate_scope(scope)?;
        self.state(scope)?;
        self.selected_scopes
            .lock()
            .expect("browser selected scopes")
            .insert(scope.task_id.clone(), scope.clone());
        Ok(())
    }

    pub(crate) fn bind_selected_agent(
        &self,
        session: &str,
        task: &lilia_contracts::TaskId,
    ) -> Result<(), BrowserError> {
        let scope = self
            .selected_scopes
            .lock()
            .expect("browser selected scopes")
            .get(task)
            .cloned();
        if let Some(scope) = scope {
            self.bind_agent(session.to_owned(), scope)?;
        }
        Ok(())
    }

    pub(crate) fn bind_agent(
        &self,
        session: String,
        scope: BrowserScope,
    ) -> Result<(), BrowserError> {
        self.validate_scope(&scope)?;
        self.state(&scope)?;
        let previous = self
            .agent_scopes
            .lock()
            .expect("browser agent scopes")
            .insert(session.clone(), scope.clone());
        if previous.as_ref() != Some(&scope) {
            self.private_inputs
                .lock()
                .expect("browser private inputs")
                .discard_session(&session);
        }
        Ok(())
    }

    pub(crate) fn agent_scope(&self, session: &str) -> Result<BrowserScope, BrowserError> {
        self.agent_scopes
            .lock()
            .expect("browser agent scopes")
            .get(session)
            .cloned()
            .ok_or(BrowserError::WrongScope)
    }

    pub(crate) fn cancel_agent(&self, session: &str, turn: &str) {
        self.private_inputs
            .lock()
            .expect("browser private inputs")
            .discard_session(session);
        self.cancelled_turns
            .lock()
            .expect("browser cancelled turns")
            .insert((session.to_owned(), turn.to_owned()));
        if let Ok(scope) = self.agent_scope(session) {
            let _ = self.cancel(&scope);
        }
    }

    pub fn register(
        &self,
        scope: BrowserScope,
        page: BrowserPage,
    ) -> Result<BrowserState, BrowserError> {
        if scope.tab_id.trim().is_empty() {
            return Err(BrowserError::UnknownTab);
        }
        self.validate_scope(&scope)?;
        let mut tabs = self.tabs.lock().expect("browser sessions");
        if tabs.contains_key(&scope.tab_id) {
            return Err(BrowserError::Busy);
        }
        let state = BrowserState {
            scope,
            lifecycle: self.next_lifecycle.fetch_add(1, Ordering::Relaxed),
            page_version: 1,
            control: BrowserControl::Agent,
            page,
        };
        tabs.insert(
            state.scope.tab_id.clone(),
            Tab {
                state: state.clone(),
                active: None,
            },
        );
        Ok(state)
    }

    pub fn state(&self, scope: &BrowserScope) -> Result<BrowserState, BrowserError> {
        let tabs = self.tabs.lock().expect("browser sessions");
        Ok(Self::tab(&tabs, scope)?.state.clone())
    }

    /// Whether an Agent operation is currently executing for this tab.
    ///
    /// This is intentionally separate from `BrowserState`: busy is a
    /// transient host concern and must not be persisted as page state.
    pub fn is_busy(&self, scope: &BrowserScope) -> bool {
        self.tabs
            .lock()
            .ok()
            .and_then(|tabs| Self::tab(&tabs, scope).ok().map(|tab| tab.active.is_some()))
            .unwrap_or(false)
    }

    fn tab<'a>(
        tabs: &'a BTreeMap<String, Tab>,
        scope: &BrowserScope,
    ) -> Result<&'a Tab, BrowserError> {
        let tab = tabs.get(&scope.tab_id).ok_or(BrowserError::UnknownTab)?;
        if tab.state.scope != *scope {
            return Err(BrowserError::WrongScope);
        }
        Ok(tab)
    }

    pub fn execute(&self, request: &BrowserRequest) -> Result<BrowserState, BrowserError> {
        self.execute_scoped(request, None)
    }

    pub(crate) fn execute_agent(
        &self,
        session: &str,
        turn: &str,
        request: &BrowserRequest,
    ) -> Result<BrowserState, BrowserError> {
        self.execute_scoped(request, Some((session, turn)))
    }

    fn execute_scoped(
        &self,
        request: &BrowserRequest,
        agent: Option<(&str, &str)>,
    ) -> Result<BrowserState, BrowserError> {
        self.validate_scope(&request.scope)?;
        let token = BrowserCancellation(
            Arc::new(AtomicBool::new(false)),
            Some(Arc::new(BrowserAuthorityGuard {
                authority: self.authority.clone().ok_or(BrowserError::Unavailable)?,
                scope: request.scope.clone(),
            })),
        );
        {
            let mut tabs = self.tabs.lock().expect("browser sessions");
            if let Some((session, turn)) = agent {
                if self
                    .cancelled_turns
                    .lock()
                    .expect("browser cancelled turns")
                    .contains(&(session.to_owned(), turn.to_owned()))
                {
                    return Err(BrowserError::Cancelled);
                }
            }
            let tab = Self::tab(&tabs, &request.scope)?;
            if tab.state.lifecycle != request.lifecycle {
                return Err(BrowserError::StaleLifecycle);
            }
            if tab.state.control != BrowserControl::Agent {
                return Err(BrowserError::HumanControl);
            }
            if tab.state.page_version != request.page_version {
                return Err(BrowserError::StalePage);
            }
            if let lilia_contracts::BrowserOperation::Click { target }
            | lilia_contracts::BrowserOperation::Type { target, .. } = &request.operation
            {
                if !tab
                    .state
                    .page
                    .targets
                    .iter()
                    .any(|entry| entry.id == *target)
                {
                    return Err(BrowserError::StalePage);
                }
            }
            if tab.active.is_some() {
                return Err(BrowserError::Busy);
            }
            tabs.get_mut(&request.scope.tab_id).unwrap().active = Some(token.clone());
        }
        let result = self.host.execute(request, &token);
        if let Err(error) = self.validate_scope(&request.scope) {
            self.release_active(&request.scope, &token);
            return Err(error);
        }
        let mut result = match result {
            Ok(result) => result,
            Err(error) => {
                self.release_active(&request.scope, &token);
                if matches!(error, BrowserError::Cancelled) {
                    if let Ok(mut tabs) = self.tabs.lock() {
                        if let Some(tab) = tabs.get_mut(&request.scope.tab_id) {
                            if tab.state.lifecycle == request.lifecycle
                                && tab.state.page_version == request.page_version
                            {
                                tab.state.page_version += 1;
                                tab.state.page.targets.clear();
                            }
                        }
                    }
                }
                return Err(error);
            }
        };
        if matches!(
            request.operation,
            lilia_contracts::BrowserOperation::Screenshot
        ) {
            let _artifact_commit = self
                .artifact_commit
                .lock()
                .expect("browser artifact commit lock");
            let (session, turn) = match agent {
                Some(value) => value,
                None => {
                    self.release_active(&request.scope, &token);
                    return Err(BrowserError::Unavailable);
                }
            };
            if token.is_cancelled() {
                self.release_active(&request.scope, &token);
                return Err(BrowserError::Cancelled);
            }
            let bytes = match result.screenshot_bytes.take() {
                Some(bytes) => bytes,
                None => {
                    self.release_active(&request.scope, &token);
                    return Err(BrowserError::Host("Screenshot is unavailable".into()));
                }
            };
            let sink = self.artifact_sink.as_ref();
            let resource = match sink {
                Some(sink) => sink.record_screenshot(&request.scope, session, turn, &bytes, &token),
                None => Err(BrowserError::Unavailable),
            };
            let resource = match resource {
                Ok(resource) => resource,
                Err(error) => {
                    self.release_active(&request.scope, &token);
                    return Err(error);
                }
            };
            result.screenshot_artifact = Some(resource);
            result.screenshot_bytes = None;
        }
        let mut tabs = self.tabs.lock().expect("browser sessions");
        let tab = tabs
            .get_mut(&request.scope.tab_id)
            .ok_or(BrowserError::Cancelled)?;
        if token.is_cancelled()
            || tab.state.scope != request.scope
            || tab.state.lifecycle != request.lifecycle
        {
            if tab.state.lifecycle == request.lifecycle
                && tab.state.page_version == request.page_version
            {
                tab.state.page_version += 1;
                tab.state.page.targets.clear();
            }
            if tab
                .active
                .as_ref()
                .is_some_and(|active| Arc::ptr_eq(&active.0, &token.0))
            {
                tab.active = None;
            }
            return Err(BrowserError::Cancelled);
        }
        tab.active = None;
        if tab.state.page_version != request.page_version {
            return Err(BrowserError::StalePage);
        }
        // Even a failed input may have affected the page before the host reports failure.
        tab.state.page_version += 1;
        tab.state.page.targets.clear();
        tab.state.page = result;
        Ok(tab.state.clone())
    }

    pub fn page_changed(
        &self,
        scope: &BrowserScope,
        page: BrowserPage,
    ) -> Result<BrowserState, BrowserError> {
        self.validate_scope(scope)?;
        let _artifact_commit = self
            .artifact_commit
            .lock()
            .expect("browser artifact commit lock");
        let state = {
            let mut tabs = self.tabs.lock().expect("browser sessions");
            Self::tab(&tabs, scope)?;
            let tab = tabs.get_mut(&scope.tab_id).unwrap();
            if let Some(token) = tab.active.take() {
                token.cancel();
            }
            tab.state.page_version += 1;
            tab.state.page = page;
            tab.state.clone()
        };
        self.host.cancel(scope);
        Ok(state)
    }

    pub fn takeover(&self, scope: &BrowserScope) -> Result<BrowserState, BrowserError> {
        self.validate_scope(scope)?;
        self.invalidate(scope, Some(BrowserControl::Human))
    }

    pub fn resume(&self, scope: &BrowserScope) -> Result<BrowserState, BrowserError> {
        self.validate_scope(scope)?;
        self.invalidate(scope, Some(BrowserControl::Agent))
    }

    pub fn cancel(&self, scope: &BrowserScope) -> Result<BrowserState, BrowserError> {
        self.invalidate(scope, None)
    }

    fn invalidate(
        &self,
        scope: &BrowserScope,
        control: Option<BrowserControl>,
    ) -> Result<BrowserState, BrowserError> {
        let _artifact_commit = self
            .artifact_commit
            .lock()
            .expect("browser artifact commit lock");
        self.private_inputs
            .lock()
            .expect("browser private inputs")
            .discard_scope(scope);
        let state = {
            let mut tabs = self.tabs.lock().expect("browser sessions");
            Self::tab(&tabs, scope)?;
            let tab = tabs.get_mut(&scope.tab_id).unwrap();
            if let Some(token) = tab.active.take() {
                token.cancel();
            }
            tab.state.lifecycle = self.next_lifecycle.fetch_add(1, Ordering::Relaxed);
            tab.state.page_version += 1;
            tab.state.page.targets.clear();
            if let Some(control) = control {
                tab.state.control = control;
            }
            tab.state.clone()
        };
        self.host.cancel(scope);
        Ok(state)
    }

    pub fn close(&self, scope: &BrowserScope) -> Result<(), BrowserError> {
        let _artifact_commit = self
            .artifact_commit
            .lock()
            .expect("browser artifact commit lock");
        self.selected_scopes
            .lock()
            .expect("browser selected scopes")
            .retain(|_, current| current != scope);
        self.agent_scopes
            .lock()
            .expect("browser agent scopes")
            .retain(|_, current| current != scope);
        self.private_inputs
            .lock()
            .expect("browser private inputs")
            .discard_scope(scope);
        {
            let mut tabs = self.tabs.lock().expect("browser sessions");
            Self::tab(&tabs, scope)?;
            if let Some(token) = tabs.remove(&scope.tab_id).unwrap().active {
                token.cancel();
            }
        }
        self.host.cancel(scope);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lilia_contracts::{BrowserOperation, ProjectId, TaskId};
    use std::sync::mpsc;

    fn scope(task: &str) -> BrowserScope {
        BrowserScope {
            project_id: ProjectId::new("project").unwrap(),
            task_id: TaskId::new(task).unwrap(),
            tab_id: "tab".into(),
        }
    }
    fn page(title: &str) -> BrowserPage {
        BrowserPage {
            url: "https://example.test".into(),
            title: title.into(),
            targets: vec![],
            screenshot_artifact: None,
            screenshot_bytes: None,
        }
    }
    fn request(state: &BrowserState) -> BrowserRequest {
        BrowserRequest {
            scope: state.scope.clone(),
            lifecycle: state.lifecycle,
            page_version: state.page_version,
            operation: BrowserOperation::Observe,
        }
    }
    struct Immediate;
    impl TaskBrowserHost for Immediate {
        fn execute(
            &self,
            _: &BrowserRequest,
            _: &BrowserCancellation,
        ) -> Result<BrowserPage, BrowserError> {
            Ok(page("updated"))
        }
        fn cancel(&self, _: &BrowserScope) {}
    }

    #[test]
    fn newly_created_agent_binds_selected_task_tab_without_another_ui_notification() {
        let sessions = BrowserSessions::for_test(Arc::new(Immediate));
        let first = sessions.register(scope("one"), page("first")).unwrap();
        sessions.select_agent_scope(&first.scope).unwrap();
        sessions
            .bind_selected_agent("new-session", &first.scope.task_id)
            .unwrap();
        assert_eq!(sessions.agent_scope("new-session").unwrap(), first.scope);
        sessions
            .bind_selected_agent("other-task", &TaskId::new("two").unwrap())
            .unwrap();
        assert_eq!(
            sessions.agent_scope("other-task"),
            Err(BrowserError::WrongScope)
        );
        sessions.close(&first.scope).unwrap();
        sessions
            .bind_selected_agent("after-close", &first.scope.task_id)
            .unwrap();
        assert_eq!(
            sessions.agent_scope("after-close"),
            Err(BrowserError::WrongScope)
        );
        assert_eq!(
            sessions.agent_scope("new-session"),
            Err(BrowserError::WrongScope)
        );
    }

    #[test]
    fn page_versions_and_scope_protect_targets_and_task_isolation() {
        let sessions = BrowserSessions::for_test(Arc::new(Immediate));
        let first = sessions.register(scope("one"), page("first")).unwrap();
        assert_eq!(sessions.state(&scope("two")), Err(BrowserError::WrongScope));
        let next = sessions.execute(&request(&first)).unwrap();
        assert!(next.page_version > first.page_version);
        assert_eq!(
            sessions.execute(&request(&first)),
            Err(BrowserError::StalePage)
        );
        let mut invalid_target = request(&next);
        invalid_target.operation = BrowserOperation::Click {
            target: "unobserved".into(),
        };
        assert_eq!(
            sessions.execute(&invalid_target),
            Err(BrowserError::StalePage)
        );
    }

    #[test]
    fn takeover_requires_explicit_resume_and_close_reopen_invalidates_old_events() {
        let sessions = BrowserSessions::for_test(Arc::new(Immediate));
        let first = sessions.register(scope("one"), page("first")).unwrap();
        let human = sessions.takeover(&first.scope).unwrap();
        assert_eq!(human.page.url, first.page.url);
        assert_eq!(human.page.title, first.page.title);
        assert_eq!(human.scope, first.scope);
        assert!(human.page.targets.is_empty());
        assert_eq!(
            sessions.execute(&request(&human)),
            Err(BrowserError::HumanControl)
        );
        let resumed = sessions.resume(&first.scope).unwrap();
        assert_eq!(resumed.page.url, first.page.url);
        assert_eq!(resumed.page.title, first.page.title);
        assert_eq!(resumed.scope, first.scope);
        assert!(sessions.execute(&request(&resumed)).is_ok());
        sessions.close(&first.scope).unwrap();
        sessions
            .register(first.scope.clone(), page("reopened"))
            .unwrap();
        assert_eq!(
            sessions.execute(&request(&first)),
            Err(BrowserError::StaleLifecycle)
        );
    }

    struct Blocking {
        started: mpsc::Sender<BrowserCancellation>,
        finish: Mutex<mpsc::Receiver<()>>,
    }

    struct Timeout;
    impl TaskBrowserHost for Timeout {
        fn execute(
            &self,
            _: &BrowserRequest,
            token: &BrowserCancellation,
        ) -> Result<BrowserPage, BrowserError> {
            token.cancel();
            Err(BrowserError::Cancelled)
        }
        fn cancel(&self, _: &BrowserScope) {}
    }

    #[test]
    fn host_timeout_releases_busy_state_and_invalidates_previous_page_targets() {
        let sessions = BrowserSessions::for_test(Arc::new(Timeout));
        let first = sessions.register(scope("one"), page("original")).unwrap();
        assert_eq!(
            sessions.execute(&request(&first)),
            Err(BrowserError::Cancelled)
        );
        assert_eq!(
            sessions.execute(&request(&first)),
            Err(BrowserError::StalePage)
        );
        let current = sessions.state(&first.scope).unwrap();
        assert_eq!(
            sessions.execute(&request(&current)),
            Err(BrowserError::Cancelled)
        );
    }
    impl TaskBrowserHost for Blocking {
        fn execute(
            &self,
            _: &BrowserRequest,
            token: &BrowserCancellation,
        ) -> Result<BrowserPage, BrowserError> {
            self.started.send(token.clone()).unwrap();
            self.finish.lock().unwrap().recv().unwrap();
            Ok(page("late result"))
        }
        fn cancel(&self, _: &BrowserScope) {}
    }

    #[test]
    fn takeover_cancels_inflight_work_and_late_results_do_not_replace_page() {
        let (started_tx, started_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let sessions = Arc::new(BrowserSessions::for_test(Arc::new(Blocking {
            started: started_tx,
            finish: Mutex::new(finish_rx),
        })));
        let first = sessions.register(scope("one"), page("original")).unwrap();
        let worker_sessions = sessions.clone();
        let request = request(&first);
        let worker = std::thread::spawn(move || worker_sessions.execute(&request));
        let token = started_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        sessions.takeover(&first.scope).unwrap();
        assert!(token.is_cancelled());
        finish_tx.send(()).unwrap();
        assert_eq!(worker.join().unwrap(), Err(BrowserError::Cancelled));
        assert_eq!(sessions.state(&first.scope).unwrap().page.title, "original");
    }

    #[test]
    fn explicit_cancel_reports_busy_then_releases_it_without_committing_late_page() {
        let (started_tx, started_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let sessions = Arc::new(BrowserSessions::for_test(Arc::new(Blocking {
            started: started_tx,
            finish: Mutex::new(finish_rx),
        })));
        let first = sessions
            .register(scope("cancel"), page("original"))
            .unwrap();
        let worker_sessions = sessions.clone();
        let request = request(&first);
        let worker = std::thread::spawn(move || worker_sessions.execute(&request));
        let token = started_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(sessions.is_busy(&first.scope));
        sessions.cancel(&first.scope).unwrap();
        assert!(token.is_cancelled());
        finish_tx.send(()).unwrap();
        assert_eq!(worker.join().unwrap(), Err(BrowserError::Cancelled));
        assert!(!sessions.is_busy(&first.scope));
        assert_eq!(sessions.state(&first.scope).unwrap().page.title, "original");
    }

    #[test]
    fn successful_agent_screenshot_materializes_bytes_once_and_returns_opaque_ref() {
        struct Allow;
        impl BrowserScopeAuthority for Allow {
            fn validate(&self, _: &BrowserScope) -> Result<(), BrowserError> {
                Ok(())
            }
        }
        struct ScreenshotHost;
        impl TaskBrowserHost for ScreenshotHost {
            fn execute(
                &self,
                request: &BrowserRequest,
                _: &BrowserCancellation,
            ) -> Result<BrowserPage, BrowserError> {
                assert!(matches!(request.operation, BrowserOperation::Screenshot));
                let mut result = page("captured");
                result.screenshot_bytes = Some(vec![137, 80, 78, 71]);
                Ok(result)
            }
            fn cancel(&self, _: &BrowserScope) {}
        }
        struct Sink(AtomicBool);
        impl BrowserArtifactSink for Sink {
            fn record_screenshot(
                &self,
                _: &BrowserScope,
                session: &str,
                turn: &str,
                bytes: &[u8],
                _: &BrowserCancellation,
            ) -> Result<String, BrowserError> {
                assert_eq!(session, "session");
                assert_eq!(turn, "turn");
                assert_eq!(bytes, [137, 80, 78, 71]);
                assert!(!self.0.swap(true, Ordering::AcqRel));
                Ok("resource://browser/project/task/hash.png".into())
            }
        }
        let sink = Arc::new(Sink(AtomicBool::new(false)));
        let sessions = BrowserSessions::with_authority_and_artifact_sink(
            Arc::new(ScreenshotHost),
            Arc::new(Allow),
            sink.clone(),
        );
        let initial = sessions
            .register(scope("screenshot"), page("original"))
            .unwrap();
        sessions
            .bind_agent("session".into(), initial.scope.clone())
            .unwrap();
        let mut request = request(&initial);
        request.operation = BrowserOperation::Screenshot;
        let result = sessions.execute_agent("session", "turn", &request).unwrap();
        assert_eq!(
            result.page.screenshot_artifact.as_deref(),
            Some("resource://browser/project/task/hash.png")
        );
        assert!(result.page.screenshot_bytes.is_none());
        assert!(!sessions.is_busy(&initial.scope));
    }
    #[test]
    fn authority_revocation_between_host_steps_cancels_before_input() {
        struct Authority(Arc<AtomicBool>);
        impl BrowserScopeAuthority for Authority {
            fn validate(&self, _: &BrowserScope) -> Result<(), BrowserError> {
                self.0
                    .load(Ordering::Acquire)
                    .then_some(())
                    .ok_or(BrowserError::PermissionDenied)
            }
        }
        struct Host {
            allowed: Arc<AtomicBool>,
            inputs: std::sync::atomic::AtomicUsize,
            cancelled: AtomicBool,
        }
        impl TaskBrowserHost for Host {
            fn execute(
                &self,
                _: &BrowserRequest,
                cancellation: &BrowserCancellation,
            ) -> Result<BrowserPage, BrowserError> {
                assert!(!cancellation.is_cancelled());
                self.allowed.store(false, Ordering::Release);
                let cancelled = cancellation.is_cancelled();
                self.cancelled.store(cancelled, Ordering::Release);
                if cancelled {
                    // Restoring authority cannot revive an already cancelled command.
                    self.allowed.store(true, Ordering::Release);
                    assert!(cancellation.is_cancelled());
                    self.allowed.store(false, Ordering::Release);
                    return Err(BrowserError::Cancelled);
                }
                self.inputs.fetch_add(1, Ordering::Relaxed);
                Ok(page("unexpected input"))
            }
            fn cancel(&self, _: &BrowserScope) {}
        }
        let allowed = Arc::new(AtomicBool::new(true));
        let host = Arc::new(Host {
            allowed: allowed.clone(),
            inputs: 0.into(),
            cancelled: false.into(),
        });
        let sessions = BrowserSessions::with_authority(host.clone(), Arc::new(Authority(allowed)));
        let state = sessions.register(scope("guarded"), page("first")).unwrap();
        assert_eq!(
            sessions.execute(&request(&state)),
            Err(BrowserError::PermissionDenied)
        );
        assert!(host.cancelled.load(Ordering::Acquire));
        assert_eq!(host.inputs.load(Ordering::Relaxed), 0);
        assert_eq!(sessions.state(&state.scope), Err(BrowserError::UnknownTab));
    }

    #[test]
    fn authority_revocation_rejects_before_host_and_discards_late_results() {
        struct Authority(Arc<AtomicBool>);
        impl BrowserScopeAuthority for Authority {
            fn validate(&self, _: &BrowserScope) -> Result<(), BrowserError> {
                if self.0.load(Ordering::Relaxed) {
                    Ok(())
                } else {
                    Err(BrowserError::PermissionDenied)
                }
            }
        }
        struct RevokingHost {
            allowed: Arc<AtomicBool>,
            calls: std::sync::atomic::AtomicUsize,
        }
        impl TaskBrowserHost for RevokingHost {
            fn execute(
                &self,
                _: &BrowserRequest,
                _: &BrowserCancellation,
            ) -> Result<BrowserPage, BrowserError> {
                self.calls.fetch_add(1, Ordering::Relaxed);
                self.allowed.store(false, Ordering::Relaxed);
                Ok(page("late"))
            }
            fn cancel(&self, _: &BrowserScope) {}
        }
        for revoke_before in [true, false] {
            let allowed = Arc::new(AtomicBool::new(true));
            let host = Arc::new(RevokingHost {
                allowed: allowed.clone(),
                calls: 0.into(),
            });
            let sessions =
                BrowserSessions::with_authority(host.clone(), Arc::new(Authority(allowed.clone())));
            let first = sessions.register(scope("one"), page("first")).unwrap();
            sessions
                .bind_agent("session".into(), first.scope.clone())
                .unwrap();
            if revoke_before {
                allowed.store(false, Ordering::Relaxed);
            }
            assert_eq!(
                sessions.execute(&request(&first)),
                Err(BrowserError::PermissionDenied)
            );
            assert_eq!(
                host.calls.load(Ordering::Relaxed),
                usize::from(!revoke_before)
            );
            assert_eq!(sessions.state(&first.scope), Err(BrowserError::UnknownTab));
            assert_eq!(
                sessions.agent_scope("session"),
                Err(BrowserError::WrongScope)
            );
            assert_eq!(
                sessions.register(first.scope.clone(), page("reopen")),
                Err(BrowserError::PermissionDenied)
            );
            assert_eq!(
                sessions.resume(&first.scope),
                Err(BrowserError::PermissionDenied)
            );
        }
    }
}
