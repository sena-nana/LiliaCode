//! Document and language operations share authoritative buffers and narrow state locks.
use crate::application::{
    BufferRevision, DesktopApplication, DesktopApplicationError, DocumentId, DocumentStore,
    LanguageRegistry, ProjectQuery,
};
use lilia_contracts::{Project, ProjectId};
use lilia_feature_task::ProjectTaskService;
use lilia_kernel::{
    Event, EventBus, Feature, FeatureContext, FeatureId, KernelError, ServiceKey, ServiceRef,
};
use lilia_service::ServiceAuthority;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentChangeKind {
    Opened,
    Edited,
    Saved,
    Reloaded,
    Closed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopDocumentChanged {
    pub document_id: DocumentId,
    pub revision: BufferRevision,
    pub kind: DocumentChangeKind,
}
impl Event for DesktopDocumentChanged {
    const NAME: &'static str = "lilia.document.changed";
}

#[derive(Clone)]
pub struct DesktopDocumentService {
    pub(super) documents: lilia_feature_document::SharedDocumentStore,
    pub(super) languages: lilia_feature_document::SharedLanguageRegistry,
    pub(super) language_services: Arc<Mutex<super::language_service::DesktopLanguageServiceState>>,
    pub(super) language_service_operations: Arc<Mutex<()>>,
    authority: ServiceAuthority,
    projects: ProjectTaskService,
    events: EventBus,
}
impl DesktopDocumentService {
    pub(crate) fn new(
        authority: ServiceAuthority,
        projects: ProjectTaskService,
        events: EventBus,
    ) -> Self {
        Self {
            documents: Arc::new(Mutex::new(DocumentStore::default())),
            languages: Arc::new(RwLock::new(LanguageRegistry::with_builtins())),
            language_services: Arc::new(Mutex::new(Default::default())),
            language_service_operations: Arc::new(Mutex::new(())),
            authority,
            projects,
            events,
        }
    }
    pub(super) fn authority(&self) -> &ServiceAuthority {
        &self.authority
    }
    pub(super) fn get_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Project, DesktopApplicationError> {
        Ok(self.projects.get_project(project_id)?)
    }
    pub(super) fn query_projects(
        &self,
        query: ProjectQuery,
    ) -> Result<Vec<Project>, DesktopApplicationError> {
        Ok(self.projects.query_projects(query)?)
    }
    pub(super) fn publish(
        &self,
        document_id: DocumentId,
        revision: BufferRevision,
        kind: DocumentChangeKind,
    ) {
        self.events.publish(DesktopDocumentChanged {
            document_id,
            revision,
            kind,
        });
    }
}
pub enum DocumentServiceKey {}
impl ServiceKey for DocumentServiceKey {
    type Value = DesktopDocumentService;
    const NAME: &'static str = "lilia.document.operations";
}
pub struct DocumentServiceFeature {
    service: DesktopDocumentService,
}
impl DocumentServiceFeature {
    pub fn new(service: DesktopDocumentService) -> Self {
        Self { service }
    }
}
impl Feature for DocumentServiceFeature {
    fn id(&self) -> FeatureId {
        FeatureId::new("lilia.feature.document-operations").expect("nonempty feature id")
    }
    fn provides(&self) -> Vec<ServiceRef> {
        vec![ServiceRef::of::<DocumentServiceKey>()]
    }
    fn mount(&self, cx: &mut FeatureContext<'_>) -> Result<(), KernelError> {
        cx.provide::<DocumentServiceKey>(self.service.clone())
    }
}
impl DesktopApplication {
    pub fn document_service(&self) -> DesktopDocumentService {
        self.inner.document_service.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::DocumentError;
    fn service() -> (DesktopDocumentService, lilia_kernel::Kernel) {
        let identity = uuid::Uuid::new_v4().to_string();
        let authority = ServiceAuthority::bootstrap_in_memory_named(&identity, &identity).unwrap();
        let projects = ProjectTaskService::new(
            authority.clone(),
            Arc::new(lilia_feature_task::SilentProjectTaskEvents),
        );
        let kernel = lilia_kernel::Kernel::new();
        let service = DesktopDocumentService::new(authority, projects, kernel.events().clone());
        kernel
            .mount(Arc::new(DocumentServiceFeature::new(service.clone())))
            .unwrap();
        (service, kernel)
    }
    #[test]
    fn direct_and_registered_edits_publish_committed_documents_and_keep_disk_conflicts() {
        let (service, kernel) = service();
        let mounted = kernel.service::<DocumentServiceKey>().unwrap();
        assert!(Arc::ptr_eq(&service.documents, &mounted.documents));
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("shared.txt");
        std::fs::write(&path, "original").unwrap();
        let (document, _) = service.open_document_at_path(&path).unwrap();
        let reader = service.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let subscription = kernel
            .events()
            .on::<DesktopDocumentChanged, _>(None, move |event| {
                tx.send((
                    event.clone(),
                    reader.document_snapshot(event.document_id).unwrap(),
                ))
                .unwrap();
            });
        let revision = service
            .replace_document_text(document.id, document.buffer.revision, "draft")
            .unwrap();
        let (event, observed) = rx.try_recv().unwrap();
        assert_eq!(event.kind, DocumentChangeKind::Edited);
        assert_eq!(event.revision, revision);
        assert_eq!(observed.buffer.text, "draft");
        assert!(observed.buffer.is_dirty());
        assert!(rx.try_recv().is_err());
        assert!(
            mounted
                .replace_document_text(document.id, document.buffer.revision, "stale")
                .is_err()
        );
        assert!(mounted.close_document(document.id, false).is_err());
        assert!(rx.try_recv().is_err());
        let saved = mounted.save_document(document.id, revision).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "draft");
        assert!(!saved.buffer.is_dirty());
        let (event, observed) = rx.try_recv().unwrap();
        assert_eq!(event.kind, DocumentChangeKind::Saved);
        assert_eq!(observed, saved);
        assert!(rx.try_recv().is_err());
        let revision = mounted
            .replace_document_text(document.id, revision, "next draft")
            .unwrap();
        rx.try_recv().unwrap();
        std::fs::write(&path, "outside writer").unwrap();
        assert!(matches!(
            mounted.save_document(document.id, revision),
            Err(DesktopApplicationError::Document(
                DocumentError::SaveConflict { .. }
            ))
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "outside writer");
        assert_eq!(
            service.document_snapshot(document.id).unwrap().buffer.text,
            "next draft"
        );
        assert!(rx.try_recv().is_err());
        kernel.events().unsubscribe(subscription);
    }
    #[test]
    fn read_only_documents_and_duplicate_opens_never_emit_edits() {
        let (service, kernel) = service();
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("readonly.txt");
        let (document, _) = service
            .open_document(&path, "original", None, true)
            .unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let subscription = kernel
            .events()
            .on::<DesktopDocumentChanged, _>(None, move |event| {
                tx.send(event.clone()).unwrap();
            });
        assert!(
            !service
                .open_document(&path, "replacement", None, false)
                .unwrap()
                .1
        );
        assert!(matches!(
            service.replace_document_text(document.id, document.buffer.revision, "new"),
            Err(DesktopApplicationError::Document(DocumentError::ReadOnly(
                _
            )))
        ));
        assert_eq!(service.document_snapshot(document.id).unwrap(), document);
        assert!(rx.try_recv().is_err());
        kernel.events().unsubscribe(subscription);
    }
}
