use super::*;
use lilia_contracts::{BrowserHostDecision, BrowserHostRequest, BrowserHostRequestKind};
use std::rc::Weak as RcWeak;

pub(super) enum Payload {
    Download(ICoreWebView2DownloadStartingEventArgs),
    NewWindow(ICoreWebView2NewWindowRequestedEventArgs),
    Upload { node: u64, multiple: bool },
}

pub(super) struct Download {
    ticket: BrowserHostRequest,
    tab: RcWeak<Tab>,
    pub operation: ICoreWebView2DownloadOperation,
    epoch: u64,
}

pub(super) struct Upload {
    ticket: BrowserHostRequest,
    tab: Rc<Tab>,
    node: u64,
    token: BrowserCancellation,
    epoch: u64,
    issued: Instant,
}

pub(super) struct Pending {
    pub ticket: BrowserHostRequest,
    pub payload: Payload,
    deferral: Option<ICoreWebView2Deferral>,
    tab: Rc<Tab>,
    epoch: u64,
    issued: Instant,
    upload_dispatched: bool,
}

fn epoch(tab: &Tab, kind: &BrowserHostRequestKind) -> u64 {
    if matches!(kind, BrowserHostRequestKind::Upload { .. }) {
        tab.document_epoch.get()
    } else {
        tab.navigation_epoch.get()
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if !self.upload_dispatched {
            if let Payload::Upload { node, .. } = &self.payload {
                // With Page.setInterceptFileChooserDialog enabled, the page
                // is paused until the chooser is completed.  Dropping the
                // host ticket must explicitly finish that chooser; otherwise
                // reject, timeout, navigation, and tab close can leave the
                // document waiting forever for file input.
                clear_file_input(&self.tab, *node);
            }
        }
        if let Some(deferral) = self.deferral.take() {
            unsafe {
                match &self.payload {
                    Payload::Download(args) => {
                        let _ = args.SetCancel(true);
                    }
                    Payload::NewWindow(args) => {
                        let _ = args.SetHandled(true);
                    }
                    Payload::Upload { .. } => {}
                }
                let _ = deferral.Complete();
            }
        }
    }
}

const REQUEST_TIMEOUT: Duration = Duration::from_secs(600);

fn clear_file_input(tab: &Tab, node: u64) {
    let token = BrowserCancellation::default();
    cdp(
        &tab.view,
        "DOM.setFileInputFiles",
        json!({"backendNodeId": node, "files": []}),
        &token,
        Box::new(|_| {}),
    );
}

fn cancel_upload(upload: &Upload) {
    upload.token.cancel();
    clear_file_input(&upload.tab, upload.node);
}

#[derive(Default)]
pub(super) struct Requests {
    next: u64,
    pending: BTreeMap<u64, Pending>,
    uploads: BTreeMap<u64, Upload>,
}

impl Requests {
    pub fn insert(
        &mut self,
        tab: Rc<Tab>,
        sessions: &BrowserSessions,
        kind: BrowserHostRequestKind,
        payload: Payload,
        deferral: Option<ICoreWebView2Deferral>,
    ) -> Result<BrowserHostRequest, Pending> {
        let state = sessions.state(&tab.scope).ok();
        self.next += 1;
        let ticket = BrowserHostRequest {
            id: self.next,
            scope: tab.scope.clone(),
            lifecycle: state.as_ref().map_or(0, |state| state.lifecycle),
            page_version: state.as_ref().map_or(0, |state| state.page_version),
            kind,
        };
        let pending = Pending {
            ticket: ticket.clone(),
            payload,
            deferral,
            epoch: epoch(&tab, &ticket.kind),
            tab,
            issued: Instant::now(),
            upload_dispatched: false,
        };
        if state.is_none() || self.pending.len() + self.uploads.len() >= 64 {
            return Err(pending);
        }
        self.pending.insert(ticket.id, pending);
        Ok(ticket)
    }

    pub fn tickets(&self) -> Vec<BrowserHostRequest> {
        self.pending
            .values()
            .map(|pending| pending.ticket.clone())
            .collect()
    }

    fn remove_matching(&mut self, ticket: &BrowserHostRequest) -> Option<Pending> {
        if self
            .pending
            .get(&ticket.id)
            .is_some_and(|pending| pending.ticket == *ticket)
        {
            self.pending.remove(&ticket.id)
        } else {
            None
        }
    }

    pub fn valid(
        &self,
        ticket: &BrowserHostRequest,
        sessions: &BrowserSessions,
    ) -> Result<(), BrowserError> {
        let pending = self
            .pending
            .get(&ticket.id)
            .ok_or(BrowserError::Cancelled)?;
        if pending.ticket != *ticket {
            return Err(BrowserError::WrongScope);
        }
        let state = sessions.state(&ticket.scope)?;
        if state.lifecycle != ticket.lifecycle {
            return Err(BrowserError::StaleLifecycle);
        }
        if state.page_version != ticket.page_version {
            return Err(BrowserError::StalePage);
        }
        if pending.epoch != epoch(&pending.tab, &pending.ticket.kind) {
            return Err(BrowserError::StalePage);
        }
        Ok(())
    }

    pub fn sweep(&mut self, sessions: &BrowserSessions) -> Vec<Pending> {
        let expired = self
            .pending
            .iter()
            .filter_map(|(id, pending)| {
                let current = sessions.state(&pending.ticket.scope).ok();
                (!(pending.issued.elapsed() < REQUEST_TIMEOUT
                    && current.as_ref().is_some_and(|state| {
                        state.lifecycle == pending.ticket.lifecycle
                            && state.page_version == pending.ticket.page_version
                    })
                    && pending.epoch == epoch(&pending.tab, &pending.ticket.kind)))
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        let expired = expired
            .into_iter()
            .filter_map(|id| self.pending.remove(&id))
            .collect();
        expired
    }

    pub fn sweep_uploads(&mut self, sessions: &BrowserSessions) -> Vec<BrowserHostRequest> {
        let expired = self
            .uploads
            .iter()
            .filter_map(|(id, upload)| {
                let valid = sessions
                    .state(&upload.ticket.scope)
                    .is_ok_and(|state| state.lifecycle == upload.ticket.lifecycle)
                    && upload.epoch == upload.tab.document_epoch.get()
                    && upload.issued.elapsed() < REQUEST_TIMEOUT;
                (!valid).then_some(*id)
            })
            .collect::<Vec<_>>();
        expired
            .into_iter()
            .filter_map(|id| {
                let upload = self.uploads.remove(&id)?;
                cancel_upload(&upload);
                Some(upload.ticket)
            })
            .collect()
    }

    pub fn cancel_uploads(&mut self, scope: &BrowserScope) -> Vec<BrowserHostRequest> {
        let ids = self
            .uploads
            .iter()
            .filter_map(|(id, upload)| (upload.ticket.scope == *scope).then_some(*id))
            .collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|id| {
                let upload = self.uploads.remove(&id)?;
                cancel_upload(&upload);
                Some(upload.ticket)
            })
            .collect()
    }

    pub fn cancel_scope(&mut self, scope: &BrowserScope) -> Vec<Pending> {
        let ids = self
            .pending
            .iter()
            .filter_map(|(id, pending)| (pending.ticket.scope == *scope).then_some(*id))
            .collect::<Vec<_>>();
        let removed = ids
            .into_iter()
            .filter_map(|id| self.pending.remove(&id))
            .collect();
        removed
    }

    pub fn clear(&mut self) -> Vec<Pending> {
        let removed = std::mem::take(&mut self.pending).into_values().collect();
        for upload in self.uploads.values() {
            cancel_upload(upload);
        }
        self.uploads.clear();
        removed
    }
}

impl UiBrowserHost {
    pub(super) fn poll_downloads(&self) {
        let Some(sessions) = self.sessions.borrow().upgrade() else {
            return;
        };
        let tabs = self.tabs.borrow().values().cloned().collect::<Vec<_>>();
        for tab in tabs {
            let downloads = std::mem::take(&mut *tab.downloads.borrow_mut());
            let mut active = Vec::new();
            for download in downloads {
                let mut state = COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS;
                let navigation_epoch = download.tab.upgrade().map(|tab| tab.navigation_epoch.get());
                let valid = sessions
                    .state(&download.ticket.scope)
                    .is_ok_and(|state| state.lifecycle == download.ticket.lifecycle)
                    && navigation_epoch == Some(download.epoch);
                if !valid {
                    unsafe {
                        let _ = download.operation.Cancel();
                    }
                }
                let result = unsafe { download.operation.State(&mut state) };
                if valid && result.is_ok() && state == COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS {
                    active.push(download);
                    continue;
                }
                let error = if !valid {
                    Some("Download cancelled".into())
                } else if result.is_err() || state == COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED {
                    Some("Download interrupted".into())
                } else {
                    None
                };
                self.events
                    .borrow_mut()
                    .push(BrowserHostEvent::RequestResolved {
                        request: download.ticket,
                        error,
                    });
            }
            tab.downloads.borrow_mut().extend(active);
        }
    }

    pub(super) fn cancel_downloads(&self, scope: &BrowserScope) {
        if let Ok(tab) = self.tab(scope) {
            let downloads = std::mem::take(&mut *tab.downloads.borrow_mut());
            for download in downloads {
                unsafe {
                    let _ = download.operation.Cancel();
                }
                self.events
                    .borrow_mut()
                    .push(BrowserHostEvent::RequestResolved {
                        request: download.ticket,
                        error: Some("Download cancelled".into()),
                    });
            }
        }
    }
    pub fn pending_requests(&self) -> Vec<BrowserHostRequest> {
        self.requests.borrow().tickets()
    }

    pub fn respond(
        &self,
        ticket: &BrowserHostRequest,
        decision: BrowserHostDecision,
    ) -> Result<(), BrowserError> {
        let sessions = self
            .sessions
            .borrow()
            .upgrade()
            .ok_or(BrowserError::Unavailable)?;
        if matches!(decision, BrowserHostDecision::Reject) {
            if !self
                .requests
                .borrow()
                .pending
                .get(&ticket.id)
                .is_some_and(|pending| pending.ticket == *ticket)
            {
                return Err(BrowserError::Cancelled);
            }
        } else {
            if let Err(error) = sessions.validate_scope(&ticket.scope) {
                let pending = self.requests.borrow_mut().remove_matching(ticket);
                if let Some(pending) = pending {
                    self.events
                        .borrow_mut()
                        .push(BrowserHostEvent::RequestResolved {
                            request: pending.ticket.clone(),
                            error: Some("Browser request cancelled".into()),
                        });
                }
                return Err(error);
            }
            if let Err(error) = self.requests.borrow().valid(ticket, &sessions) {
                if matches!(
                    &error,
                    BrowserError::StalePage
                        | BrowserError::StaleLifecycle
                        | BrowserError::Cancelled
                        | BrowserError::UnknownTab
                ) {
                    let pending = self.requests.borrow_mut().remove_matching(ticket);
                    if let Some(pending) = pending {
                        drop(pending);
                        self.events
                            .borrow_mut()
                            .push(BrowserHostEvent::RequestResolved {
                                request: ticket.clone(),
                                error: Some("Browser request is no longer available".into()),
                            });
                    }
                }
                return Err(error);
            }
        }
        match decision {
            BrowserHostDecision::Reject => {
                let pending = self.requests.borrow_mut().remove_matching(ticket);
                drop(pending);
                Ok(())
            }
            BrowserHostDecision::Download { path } => {
                if path.contains('\0') {
                    return Err(BrowserError::Host("Invalid download destination".into()));
                }
                let path = PathBuf::from(path);
                if !path.is_absolute()
                    || path.is_dir()
                    || path.file_name().is_none()
                    || !path.parent().is_some_and(Path::is_dir)
                {
                    return Err(BrowserError::Host(
                        "Choose an absolute download destination in an existing folder".into(),
                    ));
                }
                if !self
                    .requests
                    .borrow()
                    .pending
                    .get(&ticket.id)
                    .is_some_and(|pending| matches!(&pending.payload, Payload::Download(_)))
                {
                    return Err(BrowserError::WrongScope);
                }
                let mut pending = self
                    .requests
                    .borrow_mut()
                    .pending
                    .remove(&ticket.id)
                    .ok_or(BrowserError::Cancelled)?;
                let Payload::Download(args) = &pending.payload else {
                    return Err(BrowserError::WrongScope);
                };
                unsafe {
                    let operation = args.DownloadOperation().map_err(host_error)?;
                    let wake = self.wake.clone();
                    let handler = StateChangedEventHandler::create(Box::new(move |_, _| {
                        wake();
                        Ok(())
                    }));
                    let mut event_token = 0;
                    operation
                        .add_StateChanged(&handler, &mut event_token)
                        .map_err(host_error)?;
                    args.SetResultFilePath(PCWSTR(wide(&path.to_string_lossy()).as_ptr()))
                        .map_err(host_error)?;
                    args.SetHandled(true).map_err(host_error)?;
                    args.SetCancel(false).map_err(host_error)?;
                    if let Some(deferral) = pending.deferral.as_ref() {
                        deferral.Complete().map_err(host_error)?;
                    }
                    pending.deferral = None;
                    pending.tab.downloads.borrow_mut().push(Download {
                        ticket: ticket.clone(),
                        tab: Rc::downgrade(&pending.tab),
                        operation,
                        epoch: pending.epoch,
                    });
                }
                Ok(())
            }
            BrowserHostDecision::NewWindow { target_scope } => {
                if target_scope.project_id != ticket.scope.project_id
                    || target_scope.task_id != ticket.scope.task_id
                    || target_scope == ticket.scope
                {
                    return Err(BrowserError::WrongScope);
                }
                sessions.validate_scope(&target_scope)?;
                let target = self.tab(&target_scope)?;
                if !target.pristine.get() {
                    return Err(BrowserError::StalePage);
                }
                let source_is_human = sessions.state(&ticket.scope)?.control
                    == lilia_contracts::BrowserControl::Human;
                {
                    let requests = self.requests.borrow();
                    let pending = requests
                        .pending
                        .get(&ticket.id)
                        .ok_or(BrowserError::Cancelled)?;
                    if !matches!(&pending.payload, Payload::NewWindow(_))
                        || pending.tab.environment.as_raw() != target.environment.as_raw()
                    {
                        return Err(BrowserError::WrongScope);
                    }
                }
                if source_is_human {
                    sessions.takeover(&target_scope)?;
                }
                let mut pending = self
                    .requests
                    .borrow_mut()
                    .pending
                    .remove(&ticket.id)
                    .ok_or(BrowserError::Cancelled)?;
                let Payload::NewWindow(args) = &pending.payload else {
                    return Err(BrowserError::WrongScope);
                };
                unsafe {
                    args.SetNewWindow(&target.view).map_err(host_error)?;
                    args.SetHandled(true).map_err(host_error)?;
                    if let Some(deferral) = pending.deferral.as_ref() {
                        deferral.Complete().map_err(host_error)?;
                    }
                    pending.deferral = None;
                }
                target.pristine.set(false);
                Ok(())
            }
            BrowserHostDecision::Upload { paths } => {
                let (tab, node, multiple) = {
                    let requests = self.requests.borrow();
                    let pending = requests
                        .pending
                        .get(&ticket.id)
                        .ok_or(BrowserError::Cancelled)?;
                    let Payload::Upload { node, multiple } = pending.payload else {
                        return Err(BrowserError::WrongScope);
                    };
                    (pending.tab.clone(), node, multiple)
                };
                if paths.is_empty()
                    || (!multiple && paths.len() != 1)
                    || paths
                        .iter()
                        .any(|path| !Path::new(path).is_absolute() || !Path::new(path).is_file())
                {
                    return Err(BrowserError::Host(
                        "Choose existing files for this upload".into(),
                    ));
                }
                let token = BrowserCancellation::default();
                self.host_tokens
                    .lock()
                    .expect("browser request tokens")
                    .insert(ticket.id, (ticket.scope.clone(), token.clone()));
                if let Err(error) = self.requests.borrow().valid(ticket, &sessions) {
                    self.host_tokens
                        .lock()
                        .expect("browser request tokens")
                        .remove(&ticket.id);
                    return Err(error);
                }
                let mut pending = self
                    .requests
                    .borrow_mut()
                    .pending
                    .remove(&ticket.id)
                    .ok_or(BrowserError::Cancelled)?;
                pending.upload_dispatched = true;
                self.requests.borrow_mut().uploads.insert(
                    ticket.id,
                    Upload {
                        ticket: ticket.clone(),
                        tab: pending.tab.clone(),
                        node,
                        token: token.clone(),
                        epoch: pending.epoch,
                        issued: pending.issued,
                    },
                );
                drop(pending);
                let requests = self.requests.clone();
                let events = self.events.clone();
                let wake = self.wake.clone();
                let host_tokens = self.host_tokens.clone();
                let ticket = ticket.clone();
                let upload_tab = tab.clone();
                let upload_sessions = self.sessions.clone();
                cdp(
                    &tab.view,
                    "DOM.setFileInputFiles",
                    json!({"backendNodeId":node,"files":paths}),
                    &token,
                    Box::new(move |result| {
                        host_tokens
                            .lock()
                            .expect("browser request tokens")
                            .remove(&ticket.id);
                        let upload = requests.borrow_mut().uploads.remove(&ticket.id);
                        let completed = upload.is_some();
                        if completed {
                            let stale = upload.as_ref().is_some_and(|upload| {
                                upload.epoch != upload.tab.document_epoch.get()
                                    || upload_sessions
                                        .borrow()
                                        .upgrade()
                                        .and_then(|sessions| {
                                            sessions.state(&upload.ticket.scope).ok()
                                        })
                                        .is_none_or(|state| {
                                            state.lifecycle != upload.ticket.lifecycle
                                        })
                            });
                            let error = result
                                .err()
                                .or(stale.then_some(BrowserError::StalePage))
                                .map(|_| "Upload failed".to_owned());
                            if error.is_some() {
                                clear_file_input(&upload_tab, node);
                            }
                            events.borrow_mut().push(BrowserHostEvent::RequestResolved {
                                request: ticket,
                                error,
                            });
                        }
                        wake();
                    }),
                );
                Ok(())
            }
        }
    }
}
