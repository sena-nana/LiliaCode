use std::collections::BTreeMap;

use lilia_contracts::ProjectId;
use lilia_kernel::{JobEvent, JobId, JobRequest, JobState, Jobs};
use serde::{Deserialize, Serialize};

use crate::application::{
    DesktopDocumentDefinitionResult, DesktopDocumentDiagnosticsSnapshot, DesktopDocumentService,
    DocumentId, DocumentSnapshot, WorkspaceItem, WorkspaceItemId, WorkspaceItemResolve,
};
use crate::document_editor::{
    DocumentEditorViewState, document_editor_cursor_offset, select_document_editor_offsets,
};
use crate::runtime_compat::HostedWindowId;
use crate::runtime_shell::{ShellDiagnosticRow, ShellDocumentSnapshot};
use crate::ui_module::{ShellEffect, UiModule, UiModuleContext, UiModuleOutcome};

/// The document domain's own message vocabulary.
#[derive(Clone, Debug, PartialEq)]
pub enum DocumentMessage {
    EditorReplaced {
        item_id: WorkspaceItemId,
        expected_revision: u64,
        value: String,
    },
    GoToDefinition {
        item_id: WorkspaceItemId,
        window_id: HostedWindowId,
    },
    OpenDefinitionTarget {
        item_id: WorkspaceItemId,
        window_id: HostedWindowId,
        index: usize,
    },
    SaveEditor(WorkspaceItemId),
    DiscardEditor(WorkspaceItemId),
    /// A diagnostics or definition job reached a terminal state. The shell
    /// forwards kernel job events for the document protocols here; the module's
    /// own job maps decide whether the event is ours.
    Job(JobEvent),
}

/// What a definition job wrote into its completion payload. The project is part
/// of the lookup, so the project travels back with the targets rather than
/// being re-derived on the shell thread.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentDefinitionOutcome {
    pub project_id: Option<ProjectId>,
    pub result: DesktopDocumentDefinitionResult,
}

pub struct DocumentsModule {
    editors: BTreeMap<WorkspaceItemId, DocumentEditorViewState>,
    opened_document: Option<DocumentSnapshot>,
    active_diagnostics_jobs: BTreeMap<JobId, DocumentId>,
    active_definition_jobs: BTreeMap<JobId, (WorkspaceItemId, HostedWindowId)>,
}

impl Default for DocumentsModule {
    fn default() -> Self {
        Self {
            editors: BTreeMap::new(),
            opened_document: None,
            active_diagnostics_jobs: BTreeMap::new(),
            active_definition_jobs: BTreeMap::new(),
        }
    }
}

impl DocumentsModule {
    pub fn feature_id() -> lilia_kernel::FeatureId {
        lilia_kernel::FeatureId::new("lilia.documents")
            .expect("the documents feature id is not blank")
    }

    /// The document the files page currently points at, which its selection
    /// highlight and preview read back.
    pub fn opened_document(&self) -> Option<&DocumentSnapshot> {
        self.opened_document.as_ref()
    }

    /// Moves the editor's selection onto a definition target. Returns false
    /// when the offsets no longer match the buffer, which the shell reports as
    /// a stale definition rather than a silent no-op.
    pub fn apply_definition_selection(
        &mut self,
        item_id: &WorkspaceItemId,
        start_offset: usize,
        end_offset: usize,
    ) -> bool {
        let selected = self
            .editors
            .get(item_id)
            .is_some_and(|state| select_document_editor_offsets(state, start_offset, end_offset));
        if !selected {
            if let Some(state) = self.editors.get_mut(item_id) {
                state.definition_error = Some("定义位置已失效，请重新查找。".to_owned());
            }
        }
        selected
    }

    pub fn set_opened_document(&mut self, document: Option<DocumentSnapshot>) {
        self.opened_document = document;
    }

    pub fn has_editor(&self, item_id: &WorkspaceItemId) -> bool {
        self.editors.contains_key(item_id)
    }

    pub fn editor(&self, item_id: &WorkspaceItemId) -> Option<&DocumentEditorViewState> {
        self.editors.get(item_id)
    }

    /// The document a workspace item's editor renders, so closing the item can
    /// decide whether its document still has another view left.
    pub fn editor_document_id(&self, item_id: &WorkspaceItemId) -> Option<DocumentId> {
        self.editors.get(item_id).map(|state| state.document_id)
    }

    pub fn ensure_editor(&mut self, item: &WorkspaceItem, snapshot: &DocumentSnapshot) {
        if let Some(existing) = self.editors.get_mut(&item.id) {
            existing.sync_from_snapshot(snapshot);
            return;
        }
        self.editors.insert(
            item.id.clone(),
            DocumentEditorViewState::from_snapshot(snapshot),
        );
    }

    /// Drops the editor states of the closed items. Documents themselves are
    /// released by the shell, which knows what other views remain.
    pub fn remove_editors(&mut self, closing: &[WorkspaceItem]) {
        for item in closing {
            self.editors.remove(&item.id);
        }
    }

    /// The first closed item with unsaved changes, for the close confirmation.
    pub fn first_dirty_title(&self, items: &[WorkspaceItem]) -> Option<String> {
        items.iter().find_map(|item| {
            self.editors
                .get(&item.id)
                .filter(|state| state.dirty)
                .map(|_| item.title.clone())
        })
    }

    /// Rebuilds the editor set from the workspace items that render documents,
    /// keeping existing states (and their buffers) when an item survives.
    pub fn sync_editors_from_items(
        &mut self,
        items: &[WorkspaceItem],
        service: &DesktopDocumentService,
    ) {
        let mut next = BTreeMap::new();
        for item in items {
            let Some(path) = item.document_path().ok().flatten() else {
                continue;
            };
            let snapshot = match self
                .editors
                .get(&item.id)
                .and_then(|state| service.document_snapshot(state.document_id).ok())
            {
                Some(snapshot) => snapshot,
                None => match service.open_document_at_path(&path) {
                    Ok((snapshot, _)) => snapshot,
                    Err(error) => {
                        eprintln!("failed to sync Native document editor: {error}");
                        continue;
                    }
                },
            };
            let state = if let Some(mut existing) = self.editors.remove(&item.id) {
                existing.sync_from_snapshot(&snapshot);
                existing
            } else {
                DocumentEditorViewState::from_snapshot(&snapshot)
            };
            next.insert(item.id.clone(), state);
        }
        self.editors = next;
    }

    /// Marks every editor of the document as checking, then submits the
    /// diagnostics job on its single-flight lane.
    pub fn start_diagnostics(
        &mut self,
        document_id: DocumentId,
        project_id: Option<ProjectId>,
        jobs: &Jobs,
    ) {
        for state in self
            .editors
            .values_mut()
            .filter(|state| state.document_id == document_id)
        {
            state.mark_diagnostics_checking();
        }
        let request = JobRequest::new(
            lilia_feature_document::DIAGNOSTICS_PROTOCOL,
            serde_json::to_value(lilia_feature_document::DiagnosticsRequest {
                document_id,
                project_id: project_id.map(|project_id| project_id.to_string()),
            })
            .expect("a diagnostics request is representable as JSON"),
        )
        .in_slot(lilia_feature_document::diagnostics_slot(document_id));

        match jobs.submit(request) {
            Ok(handle) => {
                self.active_diagnostics_jobs
                    .insert(handle.id(), document_id);
            }
            Err(error) => {
                eprintln!("failed to submit the LiliaCode document diagnostics job: {error}");
                for state in self
                    .editors
                    .values_mut()
                    .filter(|state| state.document_id == document_id)
                {
                    state.mark_diagnostics_unavailable();
                }
            }
        }
    }

    /// Looks up the cursor offset and submits the definition job, unless one
    /// for this editor is already in flight.
    pub fn start_definition(
        &mut self,
        item_id: WorkspaceItemId,
        window_id: HostedWindowId,
        jobs: &Jobs,
    ) {
        let Some(state) = self.editors.get(&item_id) else {
            return;
        };
        if state.definition_job.is_some() || state.unapplied_edit {
            return;
        }
        let document_id = state.document_id;
        let revision = state.revision;
        let Some(source_offset) = document_editor_cursor_offset(state) else {
            if let Some(state) = self.editors.get_mut(&item_id) {
                state.definition_error = Some("当前光标位置不可用。".to_owned());
            }
            return;
        };
        if let Some(state) = self.editors.get_mut(&item_id) {
            state.definition_project_id = None;
            state.definition_targets.clear();
            state.definition_message = Some("正在查找定义…".to_owned());
            state.definition_error = None;
        }
        let request = JobRequest::new(
            lilia_feature_document::DEFINITION_PROTOCOL,
            serde_json::to_value(lilia_feature_document::DefinitionRequest {
                document_id,
                revision,
                source_offset,
            })
            .expect("a definition request is representable as JSON"),
        )
        .in_slot(lilia_feature_document::definition_slot(item_id.as_str()));

        match jobs.submit(request) {
            Ok(handle) => {
                self.active_definition_jobs
                    .insert(handle.id(), (item_id.clone(), window_id));
                if let Some(state) = self.editors.get_mut(&item_id) {
                    state.definition_job = Some(handle.id());
                }
            }
            Err(error) => {
                eprintln!("failed to submit the LiliaCode document definition job: {error}");
                if let Some(state) = self.editors.get_mut(&item_id) {
                    state.definition_job = None;
                    state.definition_message = None;
                    state.definition_error = Some("无法启动定义查找，请重试。".to_owned());
                }
            }
        }
    }

    /// Applies the editor action the control emitted, writing the buffer back
    /// through the document service under an optimistic revision check.
    fn handle_editor_action(
        &mut self,
        item_id: WorkspaceItemId,
        action: String,
        service: &DesktopDocumentService,
    ) {
        let Some(state) = self.editors.get_mut(&item_id) else {
            return;
        };
        if state.read_only {
            return;
        }
        if action == state.editor.text() {
            return;
        }
        let document_id = state.document_id;
        let expected = state.revision;
        state.editor.perform(action);
        state.note_text_changed();
        state.definition_targets.clear();
        if state.unapplied_edit {
            return;
        }
        let text = state.editor.text();
        match service.replace_document_text(document_id, expected, text) {
            Ok(revision) => {
                if revision == expected {
                    return;
                }
                self.sync_views(document_id, service);
                for state in self
                    .editors
                    .values_mut()
                    .filter(|state| state.document_id == document_id)
                {
                    state.note_text_changed();
                }
                if let Some(state) = self.editors.get_mut(&item_id) {
                    state.revision = revision;
                    state.conflict_message = None;
                    state.status_message = None;
                }
            }
            Err(error) => {
                eprintln!("document edit was not applied: {error}");
                let latest = service.document_snapshot(document_id).ok();
                if let Some(state) = self.editors.get_mut(&item_id) {
                    state.retain_edit_conflict(latest.as_ref());
                    state.conflict_message = Some(
                        "更改尚未写入，已保留当前内容。请选择保留并保存或重新载入。".to_owned(),
                    );
                }
            }
        }
    }

    fn save_editor(
        &mut self,
        item_id: WorkspaceItemId,
        service: &DesktopDocumentService,
        jobs: &Jobs,
    ) {
        let Some(state) = self.editors.get(&item_id) else {
            return;
        };
        let document_id = state.document_id;
        let mut expected = state.revision;
        if state.unapplied_edit {
            match service.replace_document_text(document_id, expected, state.editor.text()) {
                Ok(revision) => {
                    expected = revision;
                    if let Some(state) = self.editors.get_mut(&item_id) {
                        state.unapplied_edit = false;
                        state.revision = revision;
                    }
                    self.sync_views(document_id, service);
                }
                Err(error) => {
                    eprintln!("retained document edit was not applied: {error}");
                    let latest = service.document_snapshot(document_id).ok();
                    if let Some(state) = self.editors.get_mut(&item_id) {
                        if let Some(snapshot) = latest {
                            state.revision = snapshot.buffer.revision;
                        }
                        state.conflict_message =
                            Some("无法保存当前更改，内容仍已保留。请检查文档后重试。".to_owned());
                    }
                    return;
                }
            }
        }
        match service.save_document(document_id, expected) {
            Ok(snapshot) => {
                self.sync_views(document_id, service);
                if let Some(state) = self.editors.get_mut(&item_id) {
                    state.conflict_message = None;
                    state.status_message = Some("已保存".to_owned());
                }
                self.opened_document = Some(snapshot);
                self.start_diagnostics(document_id, None, jobs);
            }
            Err(error) => {
                if let Some(state) = self.editors.get_mut(&item_id) {
                    state.conflict_message = Some(format!("保存失败：{error}"));
                    state.status_message = None;
                }
            }
        }
    }

    fn discard_editor(
        &mut self,
        item_id: WorkspaceItemId,
        service: &DesktopDocumentService,
        jobs: &Jobs,
    ) {
        let Some(state) = self.editors.get(&item_id) else {
            return;
        };
        let document_id = state.document_id;
        if state.unapplied_edit {
            match service.document_snapshot(document_id) {
                Ok(snapshot) => {
                    if let Some(state) = self.editors.get_mut(&item_id) {
                        state.unapplied_edit = false;
                        state.sync_from_snapshot(&snapshot);
                        state.conflict_message = None;
                        state.status_message = None;
                    }
                }
                Err(error) => {
                    if let Some(state) = self.editors.get_mut(&item_id) {
                        state.conflict_message = Some(format!("无法重新载入：{error}"));
                    }
                }
            }
            return;
        }
        match service.discard_document_changes(document_id) {
            Ok(snapshot) => {
                self.sync_views(document_id, service);
                if let Some(state) = self.editors.get_mut(&item_id) {
                    state.conflict_message = None;
                    state.status_message = Some("已丢弃未保存更改".to_owned());
                }
                self.opened_document = Some(snapshot);
                self.start_diagnostics(document_id, None, jobs);
            }
            Err(error) => {
                if let Some(state) = self.editors.get_mut(&item_id) {
                    state.conflict_message = Some(format!("无法丢弃更改：{error}"));
                }
            }
        }
    }

    /// Hands the picked definition target to the shell. Which pane or window it
    /// opens in is layout, not domain.
    fn open_definition_target(
        &mut self,
        item_id: WorkspaceItemId,
        window_id: HostedWindowId,
        index: usize,
    ) -> UiModuleOutcome {
        let Some((project_id, target)) = self.editors.get(&item_id).and_then(|state| {
            state
                .definition_project_id
                .clone()
                .zip(state.definition_targets.get(index).cloned())
        }) else {
            return UiModuleOutcome::clean();
        };
        UiModuleOutcome::effect(ShellEffect::OpenDocumentDefinition {
            source_window: window_id,
            project_id,
            target,
        })
    }

    fn apply_diagnostics_job(
        &mut self,
        job_id: JobId,
        state: JobState,
        service: &DesktopDocumentService,
    ) {
        let result = match state {
            JobState::Pending | JobState::Running { .. } => return,
            JobState::Completed { output } => {
                serde_json::from_value::<DesktopDocumentDiagnosticsSnapshot>(output)
                    .map_err(|error| error.to_string())
            }
            JobState::Failed { message } => Err(message),
            // Superseded by a newer check of the same document, which will
            // deliver the answer this one would have.
            _ => {
                self.active_diagnostics_jobs.remove(&job_id);
                return;
            }
        };
        let Some(document_id) = self.active_diagnostics_jobs.remove(&job_id) else {
            return;
        };
        self.finish_diagnostics(document_id, result, service);
    }

    fn finish_diagnostics(
        &mut self,
        document_id: DocumentId,
        result: Result<DesktopDocumentDiagnosticsSnapshot, String>,
        service: &DesktopDocumentService,
    ) {
        let snapshot = match result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                eprintln!("failed to refresh Native document diagnostics: {error}");
                match service.document_diagnostics(document_id) {
                    Ok(snapshot) => snapshot,
                    Err(snapshot_error) => {
                        eprintln!(
                            "failed to read Native document diagnostics state: {snapshot_error}"
                        );
                        for state in self
                            .editors
                            .values_mut()
                            .filter(|state| state.document_id == document_id)
                        {
                            state.mark_diagnostics_unavailable();
                        }
                        return;
                    }
                }
            }
        };
        for state in self
            .editors
            .values_mut()
            .filter(|state| state.document_id == document_id)
        {
            state.sync_diagnostics(&snapshot);
        }
    }

    fn apply_definition_job(&mut self, event: &JobEvent) -> UiModuleOutcome {
        let job_id = event.job_id;
        let result = match &event.state {
            JobState::Pending | JobState::Running { .. } => return UiModuleOutcome::clean(),
            JobState::Completed { output } => {
                serde_json::from_value::<DocumentDefinitionOutcome>(output.clone())
                    .map_err(|error| error.to_string())
            }
            JobState::Failed { message } => Err(message.clone()),
            _ => {
                self.clear_definition_job(job_id);
                return UiModuleOutcome::clean();
            }
        };
        let Some((item_id, window_id)) = self.active_definition_jobs.remove(&job_id) else {
            return UiModuleOutcome::clean();
        };
        let (project_id, result) = match result {
            Ok(outcome) => (outcome.project_id, Ok(outcome.result)),
            Err(error) => (None, Err(error)),
        };
        self.finish_definitions(item_id, window_id, job_id, project_id, result)
    }

    fn finish_definitions(
        &mut self,
        item_id: WorkspaceItemId,
        window_id: HostedWindowId,
        job_id: JobId,
        project_id: Option<ProjectId>,
        result: Result<DesktopDocumentDefinitionResult, String>,
    ) -> UiModuleOutcome {
        let mut immediate_target = None;
        let Some(state) = self.editors.get_mut(&item_id) else {
            return UiModuleOutcome::clean();
        };
        if state.definition_job != Some(job_id) {
            return UiModuleOutcome::clean();
        }
        state.definition_job = None;
        state.definition_message = None;
        match result {
            Ok(result)
                if result.source_document_id == state.document_id
                    && result.source_revision == state.revision
                    && !state.unapplied_edit =>
            {
                state.definition_project_id = project_id.clone();
                match result.targets.as_slice() {
                    [] => {
                        state.definition_targets.clear();
                        state.definition_message = Some("未找到定义。".to_owned());
                    }
                    [target] => {
                        state.definition_targets.clear();
                        immediate_target =
                            project_id.map(|project_id| (project_id, target.clone()));
                    }
                    targets => {
                        state.definition_targets = targets.to_vec();
                        state.definition_message =
                            Some(format!("找到 {} 个定义，请选择。", targets.len()));
                    }
                }
                state.definition_error = None;
            }
            Ok(_) => {
                state.definition_targets.clear();
                state.definition_error = Some("文档已变化，请重新查找定义。".to_owned());
            }
            Err(error) => {
                eprintln!("failed to resolve Native document definition: {error}");
                state.definition_targets.clear();
                state.definition_error = Some("暂时无法查找定义，请稍后重试。".to_owned());
            }
        }
        match immediate_target {
            Some((project_id, target)) => {
                UiModuleOutcome::effect(ShellEffect::OpenDocumentDefinition {
                    source_window: window_id,
                    project_id,
                    target,
                })
            }
            None => UiModuleOutcome::dirty(),
        }
    }

    /// Drops a definition job the surface will never hear about again.
    fn clear_definition_job(&mut self, job_id: JobId) {
        let Some((item_id, _)) = self.active_definition_jobs.remove(&job_id) else {
            return;
        };
        if let Some(state) = self.editors.get_mut(&item_id) {
            if state.definition_job == Some(job_id) {
                state.definition_job = None;
                state.definition_message = None;
            }
        }
    }

    fn sync_views(&mut self, document_id: DocumentId, service: &DesktopDocumentService) {
        let Ok(snapshot) = service.document_snapshot(document_id) else {
            return;
        };
        for state in self
            .editors
            .values_mut()
            .filter(|state| state.document_id == document_id)
        {
            state.sync_from_snapshot(&snapshot);
        }
    }
}

impl UiModule for DocumentsModule {
    type Projection<'a> = crate::ui_module::projection::DocumentsProjection<'a>;

    type Message = DocumentMessage;

    fn feature(&self) -> lilia_kernel::FeatureId {
        Self::feature_id()
    }

    fn reduce(&mut self, message: Self::Message, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        match message {
            DocumentMessage::EditorReplaced {
                item_id,
                expected_revision,
                value,
            } => {
                let Some(state) = self.editors.get_mut(&item_id) else {
                    return UiModuleOutcome::clean();
                };
                if state.read_only {
                    return UiModuleOutcome::clean();
                }
                if state.revision.get() != expected_revision {
                    state.editor.perform(value);
                    state.retain_edit_conflict(None);
                    state.conflict_message = Some(
                        "文档已在其他位置更新。当前输入已保留，请选择保留并保存或重新载入。"
                            .to_owned(),
                    );
                    return UiModuleOutcome::dirty();
                }
                let service = match cx
                    .kernel()
                    .service::<crate::application::DocumentServiceKey>()
                    .map_err(|error| error.to_string())
                {
                    Ok(service) => service,
                    Err(error) => return UiModuleOutcome::failed(error),
                };
                self.handle_editor_action(item_id, value, &service);
                UiModuleOutcome::dirty()
            }
            DocumentMessage::GoToDefinition { item_id, window_id } => {
                self.start_definition(item_id, window_id, cx.kernel().jobs());
                UiModuleOutcome::dirty()
            }
            DocumentMessage::OpenDefinitionTarget {
                item_id,
                window_id,
                index,
            } => self.open_definition_target(item_id, window_id, index),
            DocumentMessage::SaveEditor(item_id) => {
                let service = match cx
                    .kernel()
                    .service::<crate::application::DocumentServiceKey>()
                    .map_err(|error| error.to_string())
                {
                    Ok(service) => service,
                    Err(error) => return UiModuleOutcome::failed(error),
                };
                self.save_editor(item_id, &service, cx.kernel().jobs());
                UiModuleOutcome::dirty()
            }
            DocumentMessage::DiscardEditor(item_id) => {
                let service = match cx
                    .kernel()
                    .service::<crate::application::DocumentServiceKey>()
                    .map_err(|error| error.to_string())
                {
                    Ok(service) => service,
                    Err(error) => return UiModuleOutcome::failed(error),
                };
                self.discard_editor(item_id, &service, cx.kernel().jobs());
                UiModuleOutcome::dirty()
            }
            DocumentMessage::Job(event) => {
                let service = match cx
                    .kernel()
                    .service::<crate::application::DocumentServiceKey>()
                    .map_err(|error| error.to_string())
                {
                    Ok(service) => service,
                    Err(error) => return UiModuleOutcome::failed(error),
                };
                match event.protocol.as_str() {
                    lilia_feature_document::DIAGNOSTICS_PROTOCOL => {
                        self.apply_diagnostics_job(event.job_id, event.state, &service);
                        UiModuleOutcome::dirty()
                    }
                    lilia_feature_document::DEFINITION_PROTOCOL => {
                        self.apply_definition_job(&event)
                    }
                    _ => UiModuleOutcome::clean(),
                }
            }
        }
    }

    fn invalidate(
        &mut self,
        envelope: &lilia_kernel::EventEnvelope,
        cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        let Some(event) = envelope.downcast::<crate::application::DesktopDocumentChanged>() else {
            return UiModuleOutcome::clean();
        };
        if !self
            .editors
            .values()
            .any(|state| state.document_id == event.document_id)
        {
            return UiModuleOutcome::clean();
        }
        if event.kind == crate::application::DocumentChangeKind::Closed {
            return UiModuleOutcome::clean();
        }
        let service = match cx
            .kernel()
            .service::<crate::application::DocumentServiceKey>()
        {
            Ok(service) => service,
            Err(error) => return UiModuleOutcome::failed(error.to_string()),
        };
        self.sync_views(event.document_id, &service);
        UiModuleOutcome::dirty()
    }

    fn project_fields(&self, cx: &UiModuleContext<'_>, into: Self::Projection<'_>) {
        let Some(active_item) = cx
            .workspace()
            .and_then(|session| session.snapshot().ok())
            .and_then(|snapshot| {
                snapshot
                    .panel_layout
                    .active_workspace_item()
                    .ok()
                    .flatten()
                    .cloned()
            })
        else {
            return;
        };
        let Some(state) = self.editors.get(&active_item) else {
            return;
        };
        let item_id = &active_item;
        *into.document = Some(ShellDocumentSnapshot {
            item_id: item_id.as_str().to_owned(),
            revision: state.revision.get(),
            conflicted: state.unapplied_edit,
            title: state.path_label.clone(),
            text: state.editor.text(),
            language: state.language_label.clone(),
            status: state
                .conflict_message
                .clone()
                .or_else(|| state.status_message.clone())
                .unwrap_or_else(|| {
                    if state.dirty {
                        "未保存".to_owned()
                    } else {
                        state.language_label.clone()
                    }
                }),
            read_only: state.read_only,
            dirty: state.dirty,
            diagnostics: state
                .diagnostics
                .iter()
                .map(ShellDiagnosticRow::from)
                .collect(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::DesktopApplication;
    use std::path::PathBuf;

    struct NoopHost;

    impl crate::application::DesktopHost for NoopHost {
        fn execute(
            &self,
            _: &crate::application::DesktopHostContext,
            _: crate::application::DesktopHostAction,
        ) -> Result<crate::application::DesktopHostResult, crate::application::DesktopHostError>
        {
            Ok(crate::application::DesktopHostResult::Completed)
        }
    }

    fn application(home: &std::path::Path) -> DesktopApplication {
        let identity = format!("document-view-test-{}", uuid::Uuid::new_v4());
        let authority = lilia_service::ServiceAuthority::bootstrap_in_memory_named(
            identity.clone(),
            identity.clone(),
        )
        .unwrap();
        DesktopApplication::from_authority(
            crate::application::DesktopApplicationConfig::new(home, identity).unwrap(),
            authority,
            std::sync::Arc::new(NoopHost),
        )
        .unwrap()
    }

    #[test]
    fn deleting_all_text_updates_the_authoritative_buffer() {
        let home = tempfile::tempdir().unwrap();
        let application = application(home.path());
        let (snapshot, _) = application
            .open_document(home.path().join("empty.md"), "text", None, false)
            .unwrap();
        let mut module = DocumentsModule::default();
        let item = workspace_item("editor", "empty.md");
        module.ensure_editor(&item, &snapshot);

        module.handle_editor_action(
            item.id.clone(),
            String::new(),
            &application.document_service(),
        );

        assert_eq!(
            application
                .document_snapshot(snapshot.id)
                .unwrap()
                .buffer
                .text,
            ""
        );
        assert!(module.editors[&item.id].dirty);
    }

    #[test]
    fn a_rejected_edit_survives_refresh_and_reload_preserves_the_other_writer() {
        let home = tempfile::tempdir().unwrap();
        let application = application(home.path());
        let (snapshot, _) = application
            .open_document(home.path().join("shared.md"), "original", None, false)
            .unwrap();
        let mut module = DocumentsModule::default();
        let item = workspace_item("editor", "shared.md");
        module.ensure_editor(&item, &snapshot);
        application
            .replace_document_text(snapshot.id, snapshot.buffer.revision, "other writer")
            .unwrap();

        module.handle_editor_action(
            item.id.clone(),
            "my draft".to_owned(),
            &application.document_service(),
        );
        module.sync_views(snapshot.id, &application.document_service());
        assert_eq!(module.editors[&item.id].editor.text(), "my draft");
        assert!(module.editors[&item.id].unapplied_edit);
        assert_eq!(
            application
                .document_snapshot(snapshot.id)
                .unwrap()
                .buffer
                .text,
            "other writer"
        );

        let kernel = lilia_kernel::Kernel::new();
        module.discard_editor(
            item.id.clone(),
            &application.document_service(),
            kernel.jobs(),
        );
        assert_eq!(module.editors[&item.id].editor.text(), "other writer");
        assert!(!module.editors[&item.id].unapplied_edit);
        assert_eq!(
            application
                .document_snapshot(snapshot.id)
                .unwrap()
                .buffer
                .text,
            "other writer"
        );
    }

    #[test]
    fn stale_view_events_retain_the_draft_without_reaching_the_application() {
        let mut module = DocumentsModule::default();
        let (item, state) = editor_with("editor", "current");
        module.editors.insert(item.clone(), state);
        let kernel = lilia_kernel::Kernel::new();
        let cx = UiModuleContext::new(&kernel, nana_ui_platform::WindowId::PRIMARY);
        let outcome = module.reduce(
            DocumentMessage::EditorReplaced {
                item_id: item.clone(),
                expected_revision: 99,
                value: "stale".to_owned(),
            },
            &cx,
        );
        assert!(outcome.dirty);
        assert!(module.editors[&item].unapplied_edit);
        assert_eq!(module.editors[&item].editor.text(), "stale");
    }

    #[test]
    fn retained_drafts_do_not_accept_authoritative_diagnostics_or_definitions() {
        let mut module = DocumentsModule::default();
        let (item, mut state) = editor_with("editor", "my draft");
        let snapshot = document_snapshot("other text");
        state.retain_edit_conflict(Some(&snapshot));
        state.sync_diagnostics(&DesktopDocumentDiagnosticsSnapshot {
            document_id: snapshot.id,
            buffer_revision: snapshot.buffer.revision,
            state: crate::application::DesktopDocumentDiagnosticsState::Ready,
            diagnostics: vec![crate::application::Diagnostic {
                severity: crate::application::DiagnosticSeverity::Error,
                message: "other writer's error".to_owned(),
                start_offset: 0,
                end_offset: 5,
                source: None,
                code: None,
            }],
        });
        assert!(state.diagnostics.is_empty());
        module.editors.insert(item.clone(), state);
        let kernel = lilia_kernel::Kernel::new();
        module.start_definition(item.clone(), HostedWindowId::PRIMARY, kernel.jobs());
        assert!(module.active_definition_jobs.is_empty());
        assert_eq!(module.editors[&item].editor.text(), "my draft");
    }

    use crate::application::{
        BufferId, BufferRevision, BufferSnapshot, WorkspaceFocusTarget, WorkspaceItemCapabilities,
        WorkspaceItemKind, WorkspaceResourceId,
    };

    fn document_snapshot(text: &str) -> DocumentSnapshot {
        DocumentSnapshot {
            id: DocumentId::new(1),
            canonical_path: PathBuf::from("/tmp/notes.md"),
            language: None,
            read_only: false,
            buffer: BufferSnapshot {
                id: BufferId::new(1),
                text: text.to_owned(),
                revision: BufferRevision::INITIAL,
                saved_revision: BufferRevision::INITIAL,
            },
            disk_fingerprint: 0,
        }
    }

    fn item_id(value: &str) -> WorkspaceItemId {
        WorkspaceItemId::new(value).expect("the test item id is valid")
    }

    /// One editor whose state comes from a real snapshot, keyed by `value`.
    fn editor_with(value: &str, text: &str) -> (WorkspaceItemId, DocumentEditorViewState) {
        (
            item_id(value),
            DocumentEditorViewState::from_snapshot(&document_snapshot(text)),
        )
    }

    /// Fills the workspace-sessions slot, which the projection resolves.
    struct SessionsFeature;

    impl lilia_kernel::Feature for SessionsFeature {
        fn id(&self) -> lilia_kernel::FeatureId {
            lilia_kernel::FeatureId::new("test.sessions").expect("the test feature id is valid")
        }

        fn mount(
            &self,
            cx: &mut lilia_kernel::FeatureContext<'_>,
        ) -> Result<(), lilia_kernel::KernelError> {
            cx.provide::<crate::shell_service::WorkspaceSessionsKey>(std::sync::Arc::new(
                crate::shell_service::WorkspaceSessions::new(),
            ))?;
            Ok(())
        }
    }

    #[test]
    fn without_a_window_session_the_projection_stays_empty() {
        let kernel = lilia_kernel::Kernel::new();
        kernel
            .mount_all(vec![
                std::sync::Arc::new(SessionsFeature) as std::sync::Arc<dyn lilia_kernel::Feature>
            ])
            .expect("the sessions feature mounts");
        let mut module = DocumentsModule::default();
        let (id, state) = editor_with("item-1", "hello");
        module.editors.insert(id, state);

        let cx = UiModuleContext::new(&kernel, nana_ui_platform::WindowId::PRIMARY);
        let mut snapshot = crate::runtime_shell::empty_snapshot();
        module.project(&cx, &mut snapshot);
        assert!(
            snapshot.document.is_none(),
            "no session means no active pane, so nothing to project"
        );
    }

    #[test]
    fn stale_definition_offsets_report_a_stale_target() {
        let mut module = DocumentsModule::default();
        let (id, state) = editor_with("item-1", "hello world");
        module.editors.insert(id.clone(), state);

        assert!(module.apply_definition_selection(&id, 0, 5));
        assert!(module.editors[&id].definition_error.is_none());

        assert!(!module.apply_definition_selection(&id, 100, 110));
        assert_eq!(
            module.editors[&id].definition_error.as_deref(),
            Some("定义位置已失效，请重新查找。")
        );
    }

    #[test]
    fn closing_items_drops_their_editors_and_dirty_titles() {
        let mut module = DocumentsModule::default();
        let (dirty, mut state) = editor_with("item-1", "a");
        state.dirty = true;
        let (clean, clean_state) = editor_with("item-2", "b");
        module.editors.insert(dirty.clone(), state);
        module.editors.insert(clean, clean_state);

        assert_eq!(
            module.first_dirty_title(&[]),
            None,
            "no closed item means nothing to confirm"
        );
        let items = vec![workspace_item("item-1", "a.md")];
        assert_eq!(module.first_dirty_title(&items), Some("a.md".to_owned()));

        module.remove_editors(&items);
        assert!(!module.has_editor(&dirty));
    }

    fn workspace_item(value: &str, title: &str) -> WorkspaceItem {
        WorkspaceItem::new(
            item_id(value),
            WorkspaceResourceId::new(format!("document:{value}"))
                .expect("the resource id is valid"),
            WorkspaceItemKind::new(crate::application::DOCUMENT_WORKSPACE_ITEM_KIND)
                .expect("the kind is valid"),
            title,
            WorkspaceFocusTarget::new("editor").expect("the focus target is valid"),
            WorkspaceItemCapabilities::dockable(),
        )
        .expect("the test item is valid")
    }

    #[test]
    fn a_job_event_without_the_document_service_reports_a_failure() {
        let kernel = lilia_kernel::Kernel::new();
        let mut module = DocumentsModule::default();
        let (id, state) = editor_with("item-1", "text");
        module.editors.insert(id, state);

        let cx = UiModuleContext::new(&kernel, nana_ui_platform::WindowId::PRIMARY);
        let outcome = module.reduce(
            DocumentMessage::Job(JobEvent {
                job_id: JobId::new(7),
                protocol: lilia_feature_document::DIAGNOSTICS_PROTOCOL.to_owned(),
                slot: None,
                state: JobState::Completed {
                    output: serde_json::json!({}),
                },
            }),
            &cx,
        );
        assert!(
            outcome.error.is_some(),
            "the shell owes the module a document service"
        );
    }
}

#[cfg(test)]
use crate::ui_module::ErasedUiModule;
