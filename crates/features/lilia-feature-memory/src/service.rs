use std::sync::{Arc, Mutex, MutexGuard};

use crate::{MemoryChanged, MemoryInjectionChanged, MemorySettingsChanged};
use lilia_contracts::{ProjectId, TaskId};
use lilia_kernel::{Event, EventBus};
use lilia_storage::Db;

use super::{
    DesktopMemory, InMemoryMemorySettingsStore, MemoryInjectionState, MemorySettings,
    MemorySettingsStore, MemoryStore, MemoryStoreError, MemoryUpsertInput, SqliteMemoryStore,
};

#[derive(Clone)]
pub struct DesktopMemoryService {
    state: Arc<Mutex<DesktopMemoryServiceState>>,
    events: EventBus,
}

struct DesktopMemoryServiceState {
    records: Box<dyn MemoryStore>,
    settings: Box<dyn MemorySettingsStore>,
}

impl DesktopMemoryService {
    pub fn prepare_turn_injection(
        &self,
        task_id: &str,
        turn_id: &str,
        turn_sequence: i64,
        project_id: Option<&str>,
    ) -> Result<lilia_contracts::MemoryTurnInjection, DesktopMemoryError> {
        let mut state = self.state()?;
        let settings = state.settings.load()?.unwrap_or_default().normalized();
        Ok(state.records.prepare_turn_injection(
            task_id,
            turn_id,
            turn_sequence,
            project_id,
            &settings,
        )?)
    }
    pub fn from_db_with_settings(
        db: Db,
        settings: impl MemorySettingsStore + 'static,
    ) -> Result<Self, DesktopMemoryError> {
        Ok(Self::from_stores(SqliteMemoryStore::from_db(db)?, settings))
    }

    pub fn in_memory() -> Result<Self, DesktopMemoryError> {
        Ok(Self::from_store(SqliteMemoryStore::in_memory()?))
    }

    pub fn from_store(store: impl MemoryStore + 'static) -> Self {
        Self::from_stores(store, InMemoryMemorySettingsStore::default())
    }

    pub fn from_stores(
        store: impl MemoryStore + 'static,
        settings: impl MemorySettingsStore + 'static,
    ) -> Self {
        Self {
            events: EventBus::new(),
            state: Arc::new(Mutex::new(DesktopMemoryServiceState {
                records: Box::new(store),
                settings: Box::new(settings),
            })),
        }
    }

    pub fn with_events(mut self, events: EventBus) -> Self {
        self.events = events;
        self
    }

    fn publish(&self, event: impl Event) {
        self.events.publish(event);
    }

    fn memory_changed(&self, memory: &DesktopMemory) {
        self.publish(MemoryChanged {
            memory_id: Some(memory.id.clone()),
            project_id: memory
                .project_id
                .as_deref()
                .and_then(|id| ProjectId::new(id).ok()),
        });
    }

    fn injection_changed(&self, state: &MemoryInjectionState) {
        if let Ok(task_id) = TaskId::new(&state.task_id) {
            self.publish(MemoryInjectionChanged { task_id });
        }
    }

    pub fn list(&self, project_id: Option<&str>) -> Result<Vec<DesktopMemory>, DesktopMemoryError> {
        Ok(self.state()?.records.list(project_id)?)
    }

    pub fn memory(&self, memory_id: &str) -> Result<Option<DesktopMemory>, DesktopMemoryError> {
        Ok(self.state()?.records.memory(memory_id)?)
    }

    pub fn save(&self, input: MemoryUpsertInput) -> Result<DesktopMemory, DesktopMemoryError> {
        let memory = self.state()?.records.save(input)?;
        self.memory_changed(&memory);
        Ok(memory)
    }

    pub fn set_enabled(
        &self,
        memory_id: &str,
        enabled: bool,
    ) -> Result<DesktopMemory, DesktopMemoryError> {
        self.set_enabled_if_unmodified(memory_id, enabled, None)
    }

    pub fn set_enabled_if_unmodified(
        &self,
        memory_id: &str,
        enabled: bool,
        expected_updated_at: Option<i64>,
    ) -> Result<DesktopMemory, DesktopMemoryError> {
        let memory = self
            .state()?
            .records
            .set_enabled(memory_id, enabled, expected_updated_at)?;
        self.memory_changed(&memory);
        Ok(memory)
    }

    pub fn delete(&self, memory_id: &str) -> Result<bool, DesktopMemoryError> {
        self.delete_if_unmodified(memory_id, None)
    }

    pub fn delete_if_unmodified(
        &self,
        memory_id: &str,
        expected_updated_at: Option<i64>,
    ) -> Result<bool, DesktopMemoryError> {
        let (previous, deleted) = {
            let mut state = self.state()?;
            let previous = state.records.memory(memory_id)?;
            let deleted = state.records.delete(memory_id, expected_updated_at)?;
            (previous, deleted)
        };
        if deleted {
            self.publish(MemoryChanged {
                memory_id: Some(memory_id.to_owned()),
                project_id: previous
                    .and_then(|memory| memory.project_id)
                    .and_then(|id| ProjectId::new(id).ok()),
            });
        }
        Ok(deleted)
    }

    pub fn settings(&self) -> Result<MemorySettings, DesktopMemoryError> {
        Ok(self
            .state()?
            .settings
            .load()?
            .unwrap_or_default()
            .normalized())
    }

    pub fn save_settings(
        &self,
        settings: MemorySettings,
    ) -> Result<MemorySettings, DesktopMemoryError> {
        let settings = settings.normalized();
        self.state()?.settings.save(&settings)?;
        self.publish(MemorySettingsChanged);
        Ok(settings)
    }

    pub fn injection_state(
        &self,
        task_id: &str,
    ) -> Result<MemoryInjectionState, DesktopMemoryError> {
        Ok(self.state()?.records.injection_state(task_id)?)
    }

    pub fn set_task_enabled(
        &self,
        task_id: &str,
        enabled: bool,
    ) -> Result<MemoryInjectionState, DesktopMemoryError> {
        self.set_task_enabled_if_unmodified(task_id, enabled, None)
    }

    pub fn set_task_enabled_if_unmodified(
        &self,
        task_id: &str,
        enabled: bool,
        expected_updated_at: Option<i64>,
    ) -> Result<MemoryInjectionState, DesktopMemoryError> {
        let state =
            self.state()?
                .records
                .set_task_enabled(task_id, enabled, expected_updated_at)?;
        self.injection_changed(&state);
        Ok(state)
    }

    pub fn reset_task_cooldown(
        &self,
        task_id: &str,
    ) -> Result<MemoryInjectionState, DesktopMemoryError> {
        self.reset_task_cooldown_if_unmodified(task_id, None)
    }

    pub fn reset_task_cooldown_if_unmodified(
        &self,
        task_id: &str,
        expected_updated_at: Option<i64>,
    ) -> Result<MemoryInjectionState, DesktopMemoryError> {
        let state = self
            .state()?
            .records
            .reset_task_cooldown(task_id, expected_updated_at)?;
        self.injection_changed(&state);
        Ok(state)
    }

    fn state(&self) -> Result<MutexGuard<'_, DesktopMemoryServiceState>, DesktopMemoryError> {
        self.state
            .lock()
            .map_err(|_| DesktopMemoryError::StateUnavailable)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DesktopMemoryError {
    #[error("desktop memory state is unavailable")]
    StateUnavailable,
    #[error(transparent)]
    Store(#[from] MemoryStoreError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryScope;

    fn input() -> MemoryUpsertInput {
        MemoryUpsertInput {
            id: Some("memory-1".to_owned()),
            scope: MemoryScope::User,
            project_id: None,
            title: "Review".to_owned(),
            body: "Run the focused tests".to_owned(),
            tags: vec!["workflow".to_owned()],
            enabled: true,
            source_task_id: None,
            expected_updated_at: None,
        }
    }

    #[test]
    fn service_exposes_complete_record_lifecycle() {
        let service = DesktopMemoryService::in_memory().unwrap();
        let saved = service.save(input()).unwrap();
        assert_eq!(service.list(None).unwrap(), vec![saved.clone()]);
        assert_eq!(service.memory(&saved.id).unwrap(), Some(saved.clone()));

        let disabled = service.set_enabled(&saved.id, false).unwrap();
        assert!(!disabled.enabled);
        assert!(service.delete(&saved.id).unwrap());
        assert_eq!(service.memory(&saved.id).unwrap(), None);
    }

    #[test]
    fn service_preserves_typed_store_errors() {
        let service = DesktopMemoryService::in_memory().unwrap();
        assert!(matches!(
            service.set_enabled("missing", false),
            Err(DesktopMemoryError::Store(MemoryStoreError::MemoryNotFound {
                memory_id
            })) if memory_id == "missing"
        ));
    }

    #[test]
    fn service_normalizes_and_persists_host_owned_settings() {
        let service = DesktopMemoryService::in_memory().unwrap();
        assert_eq!(service.settings().unwrap(), MemorySettings::default());
        let saved = service
            .save_settings(MemorySettings {
                enabled: false,
                baseline_injection_enabled: false,
                cooldown_turns: 0,
            })
            .unwrap();
        assert_eq!(
            saved.cooldown_turns,
            MemorySettings::default().cooldown_turns
        );
        assert_eq!(service.settings().unwrap(), saved);
    }

    #[test]
    fn mounted_and_direct_services_publish_once_after_successful_writes() {
        let kernel = lilia_kernel::Kernel::new();
        let service = DesktopMemoryService::in_memory()
            .unwrap()
            .with_events(kernel.events().clone());
        kernel
            .mount(Arc::new(crate::MemoryFeature::new(service.clone())))
            .unwrap();
        let mounted = kernel.service::<crate::MemoryServiceKey>().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        kernel.events().observe(None, move |event| {
            tx.send(event.clone()).unwrap();
        });

        let saved = service.save(input()).unwrap();
        let changed = rx.try_recv().unwrap();
        assert_eq!(
            changed
                .downcast::<MemoryChanged>()
                .unwrap()
                .memory_id
                .as_deref(),
            Some(saved.id.as_str())
        );
        assert!(rx.try_recv().is_err());
        assert_eq!(mounted.memory(&saved.id).unwrap(), Some(saved.clone()));

        assert!(!mounted.set_enabled(&saved.id, false).unwrap().enabled);
        assert!(rx.try_recv().unwrap().is::<MemoryChanged>());
        assert!(rx.try_recv().is_err());
        assert!(mounted
            .set_enabled_if_unmodified(&saved.id, true, Some(-1))
            .is_err());
        assert!(mounted.delete_if_unmodified(&saved.id, Some(-1)).is_err());
        assert!(rx.try_recv().is_err());
        assert!(!service.memory(&saved.id).unwrap().unwrap().enabled);

        assert!(mounted.delete(&saved.id).unwrap());
        assert!(rx.try_recv().unwrap().is::<MemoryChanged>());
        assert!(!mounted.delete(&saved.id).unwrap());
        assert!(rx.try_recv().is_err());
        mounted.save_settings(MemorySettings::default()).unwrap();
        assert!(rx.try_recv().unwrap().is::<MemorySettingsChanged>());
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn injection_mutations_publish_after_commit_and_reject_stale_writes() {
        let db = Db::in_memory().unwrap();
        db.lock().execute_batch("CREATE TABLE projects (id TEXT PRIMARY KEY); CREATE TABLE tasks (id TEXT PRIMARY KEY); INSERT INTO tasks (id) VALUES ('task-1');").unwrap();
        let events = EventBus::new();
        let service = DesktopMemoryService::from_store(SqliteMemoryStore::from_db(db).unwrap())
            .with_events(events.clone());
        let reader = service.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        events.on::<MemoryInjectionChanged, _>(None, move |event| {
            tx.send(reader.injection_state(event.task_id.as_str()).unwrap())
                .unwrap();
        });
        let disabled = service.set_task_enabled("task-1", false).unwrap();
        assert_eq!(rx.try_recv().unwrap(), disabled);
        assert!(rx.try_recv().is_err());
        assert!(service
            .set_task_enabled_if_unmodified("task-1", true, Some(-1))
            .is_err());
        assert!(service
            .reset_task_cooldown_if_unmodified("task-1", Some(-1))
            .is_err());
        assert!(rx.try_recv().is_err());
        let reset = service.reset_task_cooldown("task-1").unwrap();
        assert_eq!(rx.try_recv().unwrap(), reset);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn failed_settings_write_does_not_publish() {
        let events = EventBus::new();
        let service = DesktopMemoryService::from_stores(
            SqliteMemoryStore::in_memory().unwrap(),
            CorruptSettingsStore,
        )
        .with_events(events.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        events.observe(None, move |event| {
            tx.send(event.clone()).unwrap();
        });
        assert!(service.save_settings(MemorySettings::default()).is_err());
        assert!(rx.try_recv().is_err());
    }

    struct CorruptSettingsStore;

    impl MemorySettingsStore for CorruptSettingsStore {
        fn load(&self) -> Result<Option<MemorySettings>, MemoryStoreError> {
            Err(MemoryStoreError::CorruptSettings {
                message: "injected invalid JSON".to_owned(),
            })
        }

        fn save(&mut self, _settings: &MemorySettings) -> Result<(), MemoryStoreError> {
            Err(MemoryStoreError::SettingsStorage {
                operation: "save test settings",
                message: "injected write failure".to_owned(),
            })
        }
    }

    #[test]
    fn service_preserves_typed_host_settings_errors() {
        let service = DesktopMemoryService::from_stores(
            SqliteMemoryStore::in_memory().unwrap(),
            CorruptSettingsStore,
        );
        assert!(matches!(
            service.settings(),
            Err(DesktopMemoryError::Store(
                MemoryStoreError::CorruptSettings { .. }
            ))
        ));
        assert!(matches!(
            service.save_settings(MemorySettings::default()),
            Err(DesktopMemoryError::Store(
                MemoryStoreError::SettingsStorage { .. }
            ))
        ));
    }
}
