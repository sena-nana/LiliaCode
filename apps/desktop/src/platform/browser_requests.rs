use super::*;
use lilia_contracts::{BrowserHostDecision, BrowserHostRequest, BrowserHostRequestKind};

pub(super) enum Payload {
    Download(ICoreWebView2DownloadStartingEventArgs),
    NewWindow(ICoreWebView2NewWindowRequestedEventArgs),
    Upload { node: u64, multiple: bool },
}

pub(super) struct Download {
    ticket: BrowserHostRequest,
    pub operation: ICoreWebView2DownloadOperation,
}

pub(super) struct Pending {
    pub ticket: BrowserHostRequest,
    pub payload: Payload,
    deferral: Option<ICoreWebView2Deferral>,
    tab: Rc<Tab>,
    epoch: u64,
    issued: Instant,
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

#[derive(Default)]
pub(super) struct Requests {
    next: u64,
    pending: BTreeMap<u64, Pending>,
    uploads: BTreeMap<u64, (BrowserHostRequest, BrowserCancellation)>,
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
        if sessions.state(&ticket.scope)?.lifecycle != ticket.lifecycle {
            return Err(BrowserError::StaleLifecycle);
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
                (!(pending.issued.elapsed() < Duration::from_secs(600)
                    && sessions
                        .state(&pending.ticket.scope)
                        .is_ok_and(|state| state.lifecycle == pending.ticket.lifecycle)
                    && pending.epoch == epoch(&pending.tab, &pending.ticket.kind)))
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        let expired = expired
            .into_iter()
            .filter_map(|id| self.pending.remove(&id))
            .collect();
        self.uploads.retain(|_, (ticket, token)| {
            let valid = sessions
                .state(&ticket.scope)
                .is_ok_and(|state| state.lifecycle == ticket.lifecycle);
            if !valid {
                token.cancel();
            }
            valid
        });
        expired
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
        self.uploads.retain(|_, (ticket, token)| {
            if ticket.scope == *scope {
                token.cancel();
                false
            } else {
                true
            }
        });
        removed
    }

    pub fn clear(&mut self) -> Vec<Pending> {
        let removed = std::mem::take(&mut self.pending).into_values().collect();
        for (_, token) in self.uploads.values() {
            token.cancel();
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
                let valid = sessions
                    .state(&download.ticket.scope)
                    .is_ok_and(|state| state.lifecycle == download.ticket.lifecycle);
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
                self.requests.borrow_mut().pending.remove(&ticket.id);
                return Err(error);
            }
            self.requests.borrow().valid(ticket, &sessions)?;
        }
        match decision {
            BrowserHostDecision::Reject => {
                let pending = self.requests.borrow_mut().pending.remove(&ticket.id);
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
                        operation,
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
                let target = self.tab(&target_scope)?;
                if !target.pristine.get() {
                    return Err(BrowserError::StalePage);
                }
                if sessions.state(&ticket.scope)?.control == lilia_contracts::BrowserControl::Human
                {
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
                if pending.tab.environment.as_raw() != target.environment.as_raw() {
                    return Err(BrowserError::WrongScope);
                }
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
                self.requests.borrow_mut().pending.remove(&ticket.id);
                self.requests
                    .borrow_mut()
                    .uploads
                    .insert(ticket.id, (ticket.clone(), token.clone()));
                let requests = self.requests.clone();
                let events = self.events.clone();
                let wake = self.wake.clone();
                let host_tokens = self.host_tokens.clone();
                let ticket = ticket.clone();
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
                        requests.borrow_mut().uploads.remove(&ticket.id);
                        events.borrow_mut().push(BrowserHostEvent::RequestResolved {
                            request: ticket,
                            error: result.err().map(|error| error.to_string()),
                        });
                        wake();
                    }),
                );
                Ok(())
            }
        }
    }
}
