use super::provider::{DesktopAgentRuntimeSettingsState, runtime_configuration};
use crate::application::{
    DesktopAgentRuntimeSettings, DesktopAgentRuntimeSettingsUpdate, DesktopApplication,
    DesktopApplicationError, DesktopProviderError, ProviderChanged,
};
use lilia_kernel::{
    EventBus, Feature, FeatureContext, FeatureId, KernelError, ServiceKey, ServiceRef,
};
use lilia_storage::SqliteAgentRuntimeStateStore;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

pub trait ProviderModelRuntimePort: Send + Sync {
    /// An error must leave the active runtime configuration unchanged.
    fn apply(&self, settings: &DesktopAgentRuntimeSettings) -> Result<(), String>;
}
struct SharedProviderModelRuntime(lilia_service::ServiceAuthority);
impl ProviderModelRuntimePort for SharedProviderModelRuntime {
    fn apply(&self, settings: &DesktopAgentRuntimeSettings) -> Result<(), String> {
        self.0
            .shared_runtime()
            .inner()
            .configure_model_runtime(runtime_configuration(settings))
            .map_err(|error| error.to_string())
    }
}

#[derive(Clone)]
pub struct ProviderRuntimeSettingsService {
    state: Arc<Mutex<DesktopAgentRuntimeSettingsState>>,
    runtime: Arc<dyn ProviderModelRuntimePort>,
    provider_revision: Arc<AtomicU64>,
    events: EventBus,
}
impl ProviderRuntimeSettingsService {
    pub fn open(
        store: SqliteAgentRuntimeStateStore,
        runtime: Arc<dyn ProviderModelRuntimePort>,
        provider_revision: Arc<AtomicU64>,
        events: EventBus,
    ) -> Result<Self, DesktopProviderError> {
        let state = DesktopAgentRuntimeSettingsState::open(store)?;
        runtime
            .apply(&state.current())
            .map_err(DesktopProviderError::Runtime)?;
        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            runtime,
            provider_revision,
            events,
        })
    }
    pub(crate) fn from_authority(
        store: SqliteAgentRuntimeStateStore,
        authority: lilia_service::ServiceAuthority,
        revision: Arc<AtomicU64>,
        events: EventBus,
    ) -> Result<Self, DesktopProviderError> {
        Self::open(
            store,
            Arc::new(SharedProviderModelRuntime(authority)),
            revision,
            events,
        )
    }
    pub fn settings(&self) -> Result<DesktopAgentRuntimeSettings, DesktopProviderError> {
        self.state
            .lock()
            .map(|state| state.current())
            .map_err(|_| DesktopProviderError::SettingsStateUnavailable)
    }
    pub fn save(
        &self,
        update: DesktopAgentRuntimeSettingsUpdate,
    ) -> Result<DesktopAgentRuntimeSettings, DesktopProviderError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| DesktopProviderError::SettingsStateUnavailable)?;
        let previous = state.current();
        let next = state.prepare_update(update)?;
        state.persist(&next)?;
        if let Err(message) = self.runtime.apply(&next) {
            let rollback_failed = state
                .persist(&previous)
                .err()
                .map(|error| error.to_string());
            return Err(DesktopProviderError::RuntimeSettingsApply {
                message,
                rollback_failed,
            });
        }
        state.commit(next.clone());
        let revision = self
            .provider_revision
            .fetch_add(1, Ordering::AcqRel)
            .saturating_add(1);
        drop(state);
        self.events.publish(ProviderChanged {
            provider_id: None,
            revision,
        });
        Ok(next)
    }
}

pub struct ProviderRuntimeSettingsKey;
impl ServiceKey for ProviderRuntimeSettingsKey {
    type Value = ProviderRuntimeSettingsService;
    const NAME: &'static str = "lilia.provider.runtime-settings";
}
pub struct ProviderRuntimeSettingsFeature {
    service: ProviderRuntimeSettingsService,
}
impl ProviderRuntimeSettingsFeature {
    pub fn new(service: ProviderRuntimeSettingsService) -> Self {
        Self { service }
    }
}
impl Feature for ProviderRuntimeSettingsFeature {
    fn id(&self) -> FeatureId {
        FeatureId::new("lilia.feature.provider-runtime-settings").expect("nonempty feature id")
    }
    fn provides(&self) -> Vec<ServiceRef> {
        vec![ServiceRef::of::<ProviderRuntimeSettingsKey>()]
    }
    fn mount(&self, cx: &mut FeatureContext<'_>) -> Result<(), KernelError> {
        cx.provide::<ProviderRuntimeSettingsKey>(self.service.clone())
    }
}
impl DesktopApplication {
    pub fn provider_runtime_settings_service(&self) -> ProviderRuntimeSettingsService {
        self.inner.provider_runtime_settings.clone()
    }
    pub fn provider_runtime_settings(
        &self,
    ) -> Result<DesktopAgentRuntimeSettings, DesktopApplicationError> {
        self.inner
            .provider_runtime_settings
            .settings()
            .map_err(Into::into)
    }
    pub fn save_provider_runtime_settings(
        &self,
        update: DesktopAgentRuntimeSettingsUpdate,
    ) -> Result<DesktopAgentRuntimeSettings, DesktopApplicationError> {
        self.inner
            .provider_runtime_settings
            .save(update)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    #[derive(Default)]
    struct Runtime {
        fail: AtomicBool,
        calls: AtomicU64,
        applied: Mutex<Option<DesktopAgentRuntimeSettings>>,
    }
    impl ProviderModelRuntimePort for Runtime {
        fn apply(&self, settings: &DesktopAgentRuntimeSettings) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                return Err("runtime rejected settings".into());
            }
            *self.applied.lock().unwrap() = Some(settings.clone());
            Ok(())
        }
    }
    fn update(revision: u64) -> DesktopAgentRuntimeSettingsUpdate {
        DesktopAgentRuntimeSettingsUpdate {
            expected_revision: revision,
            openai_endpoint: Some("https://models.example.test/v1".into()),
            anthropic_endpoint: None,
            model: Some("model".into()),
        }
    }

    #[test]
    fn registered_settings_publish_committed_runtime_once_and_reject_stale_without_side_effects() {
        let kernel = lilia_kernel::Kernel::new();
        let runtime = Arc::new(Runtime::default());
        let revision = Arc::new(AtomicU64::new(1));
        let service = ProviderRuntimeSettingsService::open(
            SqliteAgentRuntimeStateStore::open_in_memory().unwrap(),
            runtime.clone(),
            revision.clone(),
            kernel.events().clone(),
        )
        .unwrap();
        kernel
            .mount(Arc::new(ProviderRuntimeSettingsFeature::new(
                service.clone(),
            )))
            .unwrap();
        let registered = kernel.service::<ProviderRuntimeSettingsKey>().unwrap();
        assert!(Arc::ptr_eq(&service.state, &registered.state));
        let reader = service.clone();
        let observer = runtime.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let _subscription = kernel
            .events()
            .on::<ProviderChanged, _>(None, move |event| {
                tx.send((
                    event.revision,
                    reader.settings().unwrap(),
                    observer.applied.lock().unwrap().clone().unwrap(),
                ))
                .unwrap();
            });
        let previous = service.settings().unwrap();
        let saved = registered.save(update(previous.revision)).unwrap();
        assert_eq!(rx.try_recv().unwrap(), (2, saved.clone(), saved.clone()));
        assert!(rx.try_recv().is_err());
        let calls = runtime.calls.load(Ordering::SeqCst);
        assert!(registered.save(update(previous.revision)).is_err());
        let mut invalid = update(saved.revision);
        invalid.openai_endpoint = Some("file:///secret".into());
        assert!(registered.save(invalid).is_err());
        assert_eq!(runtime.calls.load(Ordering::SeqCst), calls);
        assert_eq!(revision.load(Ordering::SeqCst), 2);
        assert_eq!(service.settings().unwrap(), saved);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn runtime_failure_restores_durable_settings_and_emits_no_change() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("runtime.db");
        let runtime = Arc::new(Runtime::default());
        let kernel = lilia_kernel::Kernel::new();
        let service = ProviderRuntimeSettingsService::open(
            SqliteAgentRuntimeStateStore::open(&path).unwrap(),
            runtime.clone(),
            Arc::new(AtomicU64::new(1)),
            kernel.events().clone(),
        )
        .unwrap();
        let previous = service.settings().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let _subscription = kernel.events().on::<ProviderChanged, _>(None, move |_| {
            tx.send(()).unwrap();
        });
        runtime.fail.store(true, Ordering::SeqCst);
        assert!(matches!(
            service.save(update(previous.revision)),
            Err(DesktopProviderError::RuntimeSettingsApply {
                rollback_failed: None,
                ..
            })
        ));
        assert_eq!(service.settings().unwrap(), previous);
        assert_eq!(*runtime.applied.lock().unwrap(), Some(previous.clone()));
        assert!(rx.try_recv().is_err());
        let reopened = DesktopAgentRuntimeSettingsState::open(
            SqliteAgentRuntimeStateStore::open(&path).unwrap(),
        )
        .unwrap();
        assert_eq!(reopened.current(), previous);
    }

    #[test]
    fn concurrent_settings_writers_cannot_both_apply_the_same_revision() {
        let service = ProviderRuntimeSettingsService::open(
            SqliteAgentRuntimeStateStore::open_in_memory().unwrap(),
            Arc::new(Runtime::default()),
            Arc::new(AtomicU64::new(1)),
            EventBus::default(),
        )
        .unwrap();
        let revision = service.settings().unwrap().revision;
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let writers = (0..2)
            .map(|_| {
                let service = service.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    service.save(update(revision)).is_ok()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            writers
                .into_iter()
                .map(|writer| usize::from(writer.join().unwrap()))
                .sum::<usize>(),
            1
        );
        assert_eq!(service.settings().unwrap().revision, revision + 1);
    }
}
