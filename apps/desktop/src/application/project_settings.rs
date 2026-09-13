use lilia_kernel::{
    EventBus, Feature, FeatureContext, FeatureId, KernelError, ServiceKey, ServiceRef,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use lilia_storage::SqliteAgentRuntimeStateStore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::application::ProjectSettingsChanged;
use crate::application::{DesktopApplication, DesktopApplicationError};

pub const PROJECT_SETTINGS_KEY: &str = "desktop.project.settings.v1";
const PROJECT_SETTINGS_SCHEMA_VERSION: u32 = 1;

const DEFAULT_WORKTREE_AUTO_INSTRUCTIONS: &str = concat!(
    "This task is running inside a dedicated git worktree managed by Lilia.\n",
    "Keep changes scoped to this task and create commits in the worktree before requesting merge/archive."
);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DesktopWorktreeSelectionMode {
    #[default]
    Current,
    Create,
    Existing,
}

impl DesktopWorktreeSelectionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Create => "create",
            Self::Existing => "existing",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "create" => Self::Create,
            "existing" => Self::Existing,
            _ => Self::Current,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopWorktreeSettings {
    pub default_mode: DesktopWorktreeSelectionMode,
    pub parent_dir: Option<String>,
    pub auto_instructions: String,
    pub cleanup_on_archive: bool,
}

impl Default for DesktopWorktreeSettings {
    fn default() -> Self {
        Self {
            default_mode: DesktopWorktreeSelectionMode::Current,
            parent_dir: None,
            auto_instructions: DEFAULT_WORKTREE_AUTO_INSTRUCTIONS.to_owned(),
            cleanup_on_archive: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProjectSettings {
    pub clone_parent_dir: Option<String>,
    #[serde(default)]
    pub worktree: DesktopWorktreeSettings,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredProjectSettings {
    schema_version: u32,
    settings: DesktopProjectSettings,
}

#[derive(Debug, Error)]
pub enum DesktopProjectSettingsError {
    #[error("invalid project setting: {0}")]
    InvalidValue(&'static str),
    #[error("project settings persistence failed: {0}")]
    Persistence(String),
    #[error("project settings payload is corrupt: {0}")]
    Corrupt(String),
    #[error("unsupported project settings schema {0}")]
    UnsupportedSchema(u32),
}

impl From<DesktopProjectSettingsError> for DesktopApplicationError {
    fn from(error: DesktopProjectSettingsError) -> Self {
        match error {
            DesktopProjectSettingsError::InvalidValue(field) => Self::InvalidInput {
                field,
                message: "setting contains unsupported control characters".into(),
            },
            DesktopProjectSettingsError::Persistence(message)
            | DesktopProjectSettingsError::Corrupt(message) => Self::InvalidInput {
                field: "project_settings",
                message,
            },
            DesktopProjectSettingsError::UnsupportedSchema(version) => Self::InvalidInput {
                field: "project_settings",
                message: format!("unsupported schema {version}"),
            },
        }
    }
}

pub trait WorktreePreferencesPort: Send + Sync {
    fn worktree_settings(&self) -> Result<DesktopWorktreeSettings, DesktopProjectSettingsError>;
}

#[derive(Clone)]
pub struct ProjectSettingsService {
    paths: lilia_storage::LiliaDataPaths,
    store: Arc<Mutex<Option<SqliteAgentRuntimeStateStore>>>,
    events: EventBus,
}
impl ProjectSettingsService {
    pub fn new(paths: lilia_storage::LiliaDataPaths, events: EventBus) -> Self {
        Self {
            paths,
            store: Arc::new(Mutex::new(None)),
            events,
        }
    }
    fn with_store<T>(
        &self,
        operation: impl FnOnce(&SqliteAgentRuntimeStateStore) -> Result<T, DesktopProjectSettingsError>,
    ) -> Result<T, DesktopProjectSettingsError> {
        let mut store = self.store.lock().map_err(|_| {
            DesktopProjectSettingsError::Persistence("settings state unavailable".into())
        })?;
        if store.is_none() {
            self.paths
                .ensure_layout()
                .map_err(|error| DesktopProjectSettingsError::Persistence(error.to_string()))?;
            *store = Some(
                SqliteAgentRuntimeStateStore::open(self.paths.agent_runtime_db())
                    .map_err(|error| DesktopProjectSettingsError::Persistence(error.to_string()))?,
            );
        }
        operation(store.as_ref().expect("settings store initialized"))
    }
    pub fn settings(&self) -> Result<DesktopProjectSettings, DesktopProjectSettingsError> {
        self.with_store(load_project_settings)
    }
    pub fn save(
        &self,
        settings: DesktopProjectSettings,
    ) -> Result<DesktopProjectSettings, DesktopProjectSettingsError> {
        let normalized = validate_project_settings(settings)?;
        self.with_store(|store| {
            load_project_settings(store)?;
            let stored = StoredProjectSettings {
                schema_version: PROJECT_SETTINGS_SCHEMA_VERSION,
                settings: normalized.clone(),
            };
            let value = serde_json::to_value(stored)
                .map_err(|error| DesktopProjectSettingsError::Persistence(error.to_string()))?;
            store
                .put_setting(PROJECT_SETTINGS_KEY, &value)
                .map_err(|error| DesktopProjectSettingsError::Persistence(error.to_string()))
        })?;
        self.events.publish(ProjectSettingsChanged);
        Ok(normalized)
    }
    pub fn worktree_parent_directory(
        &self,
    ) -> Result<Option<PathBuf>, DesktopProjectSettingsError> {
        Ok(self.settings()?.worktree.parent_dir.map(PathBuf::from))
    }
}
impl WorktreePreferencesPort for ProjectSettingsService {
    fn worktree_settings(&self) -> Result<DesktopWorktreeSettings, DesktopProjectSettingsError> {
        Ok(self.settings()?.worktree)
    }
}

pub struct ProjectSettingsServiceKey;
impl ServiceKey for ProjectSettingsServiceKey {
    type Value = ProjectSettingsService;
    const NAME: &'static str = "lilia.project.settings";
}
pub struct ProjectSettingsFeature {
    service: ProjectSettingsService,
}
impl ProjectSettingsFeature {
    pub fn new(service: ProjectSettingsService) -> Self {
        Self { service }
    }
}
impl Feature for ProjectSettingsFeature {
    fn id(&self) -> FeatureId {
        FeatureId::new("lilia.feature.project-settings").expect("nonempty feature id")
    }
    fn provides(&self) -> Vec<ServiceRef> {
        vec![ServiceRef::of::<ProjectSettingsServiceKey>()]
    }
    fn mount(&self, cx: &mut FeatureContext<'_>) -> Result<(), KernelError> {
        cx.provide::<ProjectSettingsServiceKey>(self.service.clone())
    }
}

impl DesktopApplication {
    pub fn project_settings_service(&self) -> ProjectSettingsService {
        self.inner.project_settings.clone()
    }
    pub fn project_settings(&self) -> Result<DesktopProjectSettings, DesktopApplicationError> {
        self.inner.project_settings.settings().map_err(Into::into)
    }
    pub fn save_project_settings(
        &self,
        settings: DesktopProjectSettings,
    ) -> Result<DesktopProjectSettings, DesktopApplicationError> {
        self.inner
            .project_settings
            .save(settings)
            .map_err(Into::into)
    }
    pub fn worktree_parent_directory_preference(
        &self,
    ) -> Result<Option<PathBuf>, DesktopApplicationError> {
        self.inner
            .project_settings
            .worktree_parent_directory()
            .map_err(Into::into)
    }
    pub fn worktree_auto_instructions_for_task(
        &self,
        task_id: &lilia_contracts::TaskId,
    ) -> Result<Option<String>, DesktopApplicationError> {
        if self.task_worktree(task_id)?.is_none() {
            return Ok(None);
        }
        let text = self.project_settings()?.worktree.auto_instructions;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(trimmed.to_owned()))
        }
    }
}

fn load_project_settings(
    store: &SqliteAgentRuntimeStateStore,
) -> Result<DesktopProjectSettings, DesktopProjectSettingsError> {
    let value = store
        .setting(PROJECT_SETTINGS_KEY)
        .map_err(|error| DesktopProjectSettingsError::Persistence(error.to_string()))?;
    let Some(value) = value else {
        return Ok(DesktopProjectSettings::default());
    };
    let stored: StoredProjectSettings = serde_json::from_value(value)
        .map_err(|error| DesktopProjectSettingsError::Corrupt(error.to_string()))?;
    if stored.schema_version != PROJECT_SETTINGS_SCHEMA_VERSION {
        return Err(DesktopProjectSettingsError::UnsupportedSchema(
            stored.schema_version,
        ));
    }
    validate_project_settings(stored.settings)
}

fn validate_project_settings(
    settings: DesktopProjectSettings,
) -> Result<DesktopProjectSettings, DesktopProjectSettingsError> {
    let settings = normalize_project_settings(settings);
    for (field, value) in [
        (
            "project_settings.clone_parent_dir",
            settings.clone_parent_dir.as_deref(),
        ),
        (
            "project_settings.worktree.parent_dir",
            settings.worktree.parent_dir.as_deref(),
        ),
    ] {
        if value.is_some_and(|value| value.chars().any(char::is_control)) {
            return Err(DesktopProjectSettingsError::InvalidValue(field));
        }
    }
    if settings.worktree.auto_instructions.contains('\0') {
        return Err(DesktopProjectSettingsError::InvalidValue(
            "project_settings.worktree.auto_instructions",
        ));
    }
    Ok(settings)
}

pub fn normalize_project_settings(settings: DesktopProjectSettings) -> DesktopProjectSettings {
    DesktopProjectSettings {
        clone_parent_dir: normalize_optional_path(settings.clone_parent_dir),
        worktree: DesktopWorktreeSettings {
            default_mode: settings.worktree.default_mode,
            parent_dir: normalize_optional_path(settings.worktree.parent_dir),
            auto_instructions: {
                let trimmed = settings.worktree.auto_instructions.trim();
                if trimmed.is_empty() {
                    DEFAULT_WORKTREE_AUTO_INSTRUCTIONS.to_owned()
                } else {
                    trimmed.to_owned()
                }
            },
            cleanup_on_archive: settings.worktree.cleanup_on_archive,
        },
    }
}

pub fn default_worktree_auto_instructions() -> &'static str {
    DEFAULT_WORKTREE_AUTO_INSTRUCTIONS
}

fn normalize_optional_path(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::application::{
        DesktopApplicationConfig, DesktopHost, DesktopHostAction, DesktopHostContext,
        DesktopHostError, DesktopHostResult,
    };
    use lilia_service::ServiceAuthority;

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    struct NoopHost;

    impl DesktopHost for NoopHost {
        fn execute(
            &self,
            _context: &DesktopHostContext,
            _action: DesktopHostAction,
        ) -> Result<DesktopHostResult, DesktopHostError> {
            Ok(DesktopHostResult::Completed)
        }
    }

    fn application() -> DesktopApplication {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("lilia-project-settings-{id}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let config =
            DesktopApplicationConfig::new(&root, format!("project-settings-{id}")).unwrap();
        let authority = ServiceAuthority::bootstrap_in_memory_named(
            format!("test:project-settings:{id}"),
            format!("project-settings-test:{id}"),
        )
        .unwrap();
        DesktopApplication::from_authority(config, authority, Arc::new(NoopHost)).unwrap()
    }

    #[test]
    fn registered_settings_publish_after_durable_write_and_survive_reopen() {
        let home = tempfile::tempdir().unwrap();
        let paths = lilia_storage::LiliaDataPaths::from_home(home.path());
        let kernel = lilia_kernel::Kernel::new();
        let service = ProjectSettingsService::new(paths.clone(), kernel.events().clone());
        kernel
            .mount(Arc::new(ProjectSettingsFeature::new(service.clone())))
            .unwrap();
        let mounted = kernel.service::<ProjectSettingsServiceKey>().unwrap();
        assert!(Arc::ptr_eq(&service.store, &mounted.store));
        let reader = ProjectSettingsService::new(paths.clone(), EventBus::default());
        let (tx, rx) = std::sync::mpsc::channel();
        let _subscription = kernel
            .events()
            .on::<ProjectSettingsChanged, _>(None, move |_| {
                tx.send(reader.settings().unwrap()).unwrap();
            });
        let expected = DesktopProjectSettings {
            clone_parent_dir: Some("clones".into()),
            worktree: DesktopWorktreeSettings {
                parent_dir: Some("trees".into()),
                ..Default::default()
            },
        };
        assert_eq!(mounted.save(expected.clone()).unwrap(), expected);
        assert_eq!(rx.try_recv().unwrap(), expected);
        assert!(rx.try_recv().is_err());
        assert_eq!(
            ProjectSettingsService::new(paths, EventBus::default())
                .settings()
                .unwrap(),
            expected
        );
        assert_eq!(service.worktree_settings().unwrap(), expected.worktree);
    }

    #[test]
    fn invalid_input_and_corrupt_existing_record_are_not_written_over() {
        let home = tempfile::tempdir().unwrap();
        let paths = lilia_storage::LiliaDataPaths::from_home(home.path());
        let kernel = lilia_kernel::Kernel::new();
        let service = ProjectSettingsService::new(paths.clone(), kernel.events().clone());
        let original = service.save(DesktopProjectSettings::default()).unwrap();
        let raw = SqliteAgentRuntimeStateStore::open(paths.agent_runtime_db()).unwrap();
        let saved = raw.setting(PROJECT_SETTINGS_KEY).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let _subscription = kernel
            .events()
            .on::<ProjectSettingsChanged, _>(None, move |_| {
                tx.send(()).unwrap();
            });
        let invalid = DesktopProjectSettings {
            clone_parent_dir: Some("bad\0path".into()),
            ..original.clone()
        };
        assert!(matches!(
            service.save(invalid),
            Err(DesktopProjectSettingsError::InvalidValue(_))
        ));
        assert_eq!(raw.setting(PROJECT_SETTINGS_KEY).unwrap(), saved);
        let corrupt = serde_json::json!({"schemaVersion": 999, "settings": original});
        raw.put_setting(PROJECT_SETTINGS_KEY, &corrupt).unwrap();
        assert!(matches!(
            service.save(DesktopProjectSettings::default()),
            Err(DesktopProjectSettingsError::UnsupportedSchema(999))
        ));
        assert_eq!(raw.setting(PROJECT_SETTINGS_KEY).unwrap(), Some(corrupt));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn concurrent_full_settings_writes_never_publish_a_torn_record() {
        let home = tempfile::tempdir().unwrap();
        let kernel = lilia_kernel::Kernel::new();
        let service = ProjectSettingsService::new(
            lilia_storage::LiliaDataPaths::from_home(home.path()),
            kernel.events().clone(),
        );
        let reader = service.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let _subscription = kernel
            .events()
            .on::<ProjectSettingsChanged, _>(None, move |_| {
                tx.send(reader.settings().unwrap()).unwrap();
            });
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let writers = ["first", "second"]
            .into_iter()
            .map(|label| {
                let service = service.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    service
                        .save(DesktopProjectSettings {
                            clone_parent_dir: Some(label.into()),
                            worktree: DesktopWorktreeSettings {
                                parent_dir: Some(label.into()),
                                ..Default::default()
                            },
                        })
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let written = writers
            .into_iter()
            .map(|writer| writer.join().unwrap())
            .collect::<Vec<_>>();
        for _ in 0..2 {
            let observed = rx.try_recv().unwrap();
            assert_eq!(observed.clone_parent_dir, observed.worktree.parent_dir);
            assert!(written.contains(&observed));
        }
        assert!(rx.try_recv().is_err());
        assert!(written.contains(&service.settings().unwrap()));
    }

    #[test]
    fn project_settings_default_round_trip_and_normalize() {
        let app = application();
        let defaults = app.project_settings().unwrap();
        assert_eq!(defaults, DesktopProjectSettings::default());
        assert!(defaults.worktree.cleanup_on_archive);
        assert_eq!(
            defaults.worktree.auto_instructions,
            DEFAULT_WORKTREE_AUTO_INSTRUCTIONS
        );

        let saved = app
            .save_project_settings(DesktopProjectSettings {
                clone_parent_dir: Some("  /tmp/clones  ".into()),
                worktree: DesktopWorktreeSettings {
                    default_mode: DesktopWorktreeSelectionMode::Create,
                    parent_dir: Some("  /tmp/worktrees  ".into()),
                    auto_instructions: "  stay scoped  ".into(),
                    cleanup_on_archive: false,
                },
            })
            .unwrap();
        assert_eq!(saved.clone_parent_dir.as_deref(), Some("/tmp/clones"));
        assert_eq!(saved.worktree.parent_dir.as_deref(), Some("/tmp/worktrees"));
        assert_eq!(saved.worktree.auto_instructions, "stay scoped");
        assert!(!saved.worktree.cleanup_on_archive);
        assert_eq!(
            saved.worktree.default_mode,
            DesktopWorktreeSelectionMode::Create
        );
        assert_eq!(app.project_settings().unwrap(), saved);
        assert_eq!(
            app.worktree_parent_directory_preference().unwrap(),
            Some(PathBuf::from("/tmp/worktrees"))
        );
    }

    #[test]
    fn empty_auto_instructions_restore_default_text() {
        let app = application();
        let saved = app
            .save_project_settings(DesktopProjectSettings {
                clone_parent_dir: None,
                worktree: DesktopWorktreeSettings {
                    auto_instructions: "   ".into(),
                    ..DesktopWorktreeSettings::default()
                },
            })
            .unwrap();
        assert_eq!(
            saved.worktree.auto_instructions,
            DEFAULT_WORKTREE_AUTO_INSTRUCTIONS
        );
    }
}
