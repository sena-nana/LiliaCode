//! Durable Product Core change feed.
//!
//! Local mutations already publish in-process `DesktopEvent`s. External writers
//! (CLI, second instance, import helper) append to `product_events` without that
//! bus. This feed polls the durable event log and republishes typed invalidation
//! events so hosts re-read Product snapshots instead of treating the bus as truth.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use lilia_contracts::{
    PageRequest, ProductEntityKind, ProductEvent, ProductEventSequence, ProjectId, TaskId,
};

use lilia_kernel::{Feature, FeatureContext, FeatureId, KernelError, ServiceKey, ServiceRef};
use lilia_service::ServiceAuthority;

use crate::application::{
    AutomationChanged, DesktopApplication, DesktopApplicationError, DesktopEvent, DesktopEventBus,
    ProjectsChanged, RoadmapChanged, TasksChanged,
};

pub const PRODUCT_CHANGE_FEED_SOURCE: &str = "product-change-feed";
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(750);
const PAGE_LIMIT: u32 = 100;

pub(crate) struct ProductChangeFeedState {
    cursor: AtomicU64,
    stop: AtomicBool,
    running: AtomicBool,
    thread: Mutex<Option<JoinHandle<()>>>,
}

/// Polls durable Product events and republishes scoped UI invalidations.
///
/// The feed owns its cursor and thread lifetime. It deliberately depends on
/// the Product authority and event bus rather than the desktop composition
/// root, so external changes cannot keep the application alive indirectly.
#[derive(Clone)]
pub struct ProductChangeFeedService {
    authority: ServiceAuthority,
    events: DesktopEventBus,
    enabled: bool,
    state: Arc<ProductChangeFeedState>,
}

struct ProductChangeFeedRunGuard(ProductChangeFeedService);

impl Drop for ProductChangeFeedRunGuard {
    fn drop(&mut self) {
        self.0.state.running.store(false, Ordering::SeqCst);
    }
}

impl Default for ProductChangeFeedState {
    fn default() -> Self {
        Self {
            cursor: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            running: AtomicBool::new(false),
            thread: Mutex::new(None),
        }
    }
}

impl ProductChangeFeedService {
    pub(crate) fn new(authority: ServiceAuthority, events: DesktopEventBus) -> Self {
        let enabled = authority.data_paths().is_some();
        Self {
            authority,
            events,
            enabled,
            state: Arc::new(ProductChangeFeedState::default()),
        }
    }

    /// Advances the feed cursor to the latest durable sequence without emitting.
    /// Call before starting the poller so historical rows are not replayed.
    pub fn seed_cursor(&self) -> Result<u64, DesktopApplicationError> {
        let latest = self.latest_product_event_sequence()?;
        self.state.cursor.store(latest, Ordering::SeqCst);
        Ok(latest)
    }

    pub fn cursor(&self) -> u64 {
        self.state.cursor.load(Ordering::SeqCst)
    }

    pub fn start(&self) -> Result<(), DesktopApplicationError> {
        self.start_with_interval(DEFAULT_POLL_INTERVAL)
    }

    pub fn start_with_interval(&self, interval: Duration) -> Result<(), DesktopApplicationError> {
        if !self.enabled {
            return Ok(());
        }
        if self
            .state
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }
        self.state.stop.store(false, Ordering::SeqCst);
        if self.state.cursor.load(Ordering::SeqCst) == 0 {
            if let Err(error) = self.seed_cursor() {
                self.state.running.store(false, Ordering::SeqCst);
                return Err(error);
            }
        }
        let service = self.clone();
        let interval = interval.max(Duration::from_millis(100));
        let handle = match thread::Builder::new()
            .name("lilia-product-change-feed".to_owned())
            .spawn(move || {
                let _running = ProductChangeFeedRunGuard(service.clone());
                loop {
                    if service.state.stop.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Err(error) = service.poll() {
                        eprintln!("[product-change-feed] {error}");
                    }
                    thread::sleep(interval);
                }
            }) {
            Ok(handle) => handle,
            Err(error) => {
                self.state.running.store(false, Ordering::SeqCst);
                return Err(DesktopApplicationError::InvalidInput {
                    field: "product_change_feed",
                    message: format!("failed to start durable change feed: {error}"),
                });
            }
        };
        *self
            .state
            .thread
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(handle);
        Ok(())
    }

    pub fn stop(&self) {
        self.state.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self
            .state
            .thread
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            let _ = handle.join();
        }
        self.state.running.store(false, Ordering::SeqCst);
    }

    /// Synchronously drains durable Product events after the feed cursor.
    /// Returns the DesktopEvents that were published (may be coalesced).
    pub fn poll(&self) -> Result<Vec<DesktopEvent>, DesktopApplicationError> {
        let mut published = Vec::new();
        let mut cursor = self.state.cursor.load(Ordering::SeqCst);
        loop {
            let page = self.authority.client()?.product_events(&PageRequest {
                after: Some(ProductEventSequence::new(cursor)),
                limit: PAGE_LIMIT,
            })?;
            if page.items.is_empty() {
                break;
            }
            let kinds = coalesce_product_events(self, &page.items);
            for kind in kinds {
                published.push(self.publish_change_feed_event(kind));
            }
            cursor = page
                .items
                .last()
                .map(|event| event.sequence.get())
                .unwrap_or(cursor);
            self.state.cursor.store(cursor, Ordering::SeqCst);
            if page.next.is_none() || page.items.len() < PAGE_LIMIT as usize {
                break;
            }
        }
        Ok(published)
    }

    fn latest_product_event_sequence(&self) -> Result<u64, DesktopApplicationError> {
        let mut after = ProductEventSequence::ORIGIN;
        let mut latest = 0_u64;
        loop {
            let page = self.authority.client()?.product_events(&PageRequest {
                after: if after.get() == 0 { None } else { Some(after) },
                limit: PAGE_LIMIT,
            })?;
            if let Some(event) = page.items.last() {
                latest = event.sequence.get();
                after = event.sequence;
            }
            if page.items.is_empty() || page.next.is_none() {
                break;
            }
        }
        Ok(latest)
    }

    fn publish_change_feed_event(&self, notice: FeedNotice) -> DesktopEvent {
        match notice {
            FeedNotice::Projects => self.events.publish(ProjectsChanged),
            FeedNotice::Tasks {
                project_id,
                task_id,
            } => self.events.publish(TasksChanged {
                project_id,
                task_id,
            }),
            FeedNotice::Roadmap { project_id } => self.events.publish(RoadmapChanged {
                project_id,
                milestone_id: None,
            }),
            FeedNotice::Automation => self.events.publish(AutomationChanged {
                automation_id: None,
            }),
        }
    }
}

pub struct ProductChangeFeedServiceKey;

impl ServiceKey for ProductChangeFeedServiceKey {
    type Value = ProductChangeFeedService;
    const NAME: &'static str = "lilia.product-change-feed";
}

pub struct ProductChangeFeedFeature {
    service: ProductChangeFeedService,
}

impl ProductChangeFeedFeature {
    pub fn new(service: ProductChangeFeedService) -> Self {
        Self { service }
    }
}

impl Feature for ProductChangeFeedFeature {
    fn id(&self) -> FeatureId {
        FeatureId::new("lilia.feature.product-change-feed").expect("nonempty feature id")
    }

    fn provides(&self) -> Vec<ServiceRef> {
        vec![ServiceRef::of::<ProductChangeFeedServiceKey>()]
    }

    fn mount(&self, cx: &mut FeatureContext<'_>) -> Result<(), KernelError> {
        cx.provide::<ProductChangeFeedServiceKey>(self.service.clone())
    }
}

impl DesktopApplication {
    pub fn seed_product_change_feed_cursor(&self) -> Result<u64, DesktopApplicationError> {
        self.product_change_feed_service().seed_cursor()
    }

    pub fn product_change_feed_cursor(&self) -> u64 {
        self.product_change_feed_service().cursor()
    }

    pub fn start_product_change_feed(&self) -> Result<(), DesktopApplicationError> {
        self.product_change_feed_service().start()
    }

    pub fn start_product_change_feed_with_interval(
        &self,
        interval: Duration,
    ) -> Result<(), DesktopApplicationError> {
        self.product_change_feed_service()
            .start_with_interval(interval)
    }

    pub fn stop_product_change_feed(&self) {
        self.product_change_feed_service().stop();
    }

    pub fn poll_product_change_feed(&self) -> Result<Vec<DesktopEvent>, DesktopApplicationError> {
        self.product_change_feed_service().poll()
    }

    pub fn product_change_feed_service(&self) -> ProductChangeFeedService {
        self.inner.product_change_feed.clone()
    }
}

enum FeedNotice {
    Projects,
    Tasks {
        project_id: Option<ProjectId>,
        task_id: Option<TaskId>,
    },
    Roadmap {
        project_id: ProjectId,
    },
    Automation,
}

fn coalesce_product_events(
    service: &ProductChangeFeedService,
    events: &[ProductEvent],
) -> Vec<FeedNotice> {
    let mut projects_changed = false;
    let mut task_keys = BTreeSet::new();
    let mut roadmap_projects = BTreeSet::new();
    let mut automation_changed = false;

    for event in events {
        match event.entity.as_str() {
            "project" => projects_changed = true,
            "task" => {
                if let Ok(task_id) = TaskId::new(&event.entity_id) {
                    let project_id = task_project_id(service, &task_id);
                    task_keys.insert((project_id, Some(task_id)));
                } else {
                    projects_changed = true;
                }
            }
            "conversation" => {
                if let Some(kind) = conversation_event_kind(service, event) {
                    match kind {
                        FeedNotice::Tasks {
                            project_id,
                            task_id,
                        } => {
                            task_keys.insert((project_id, task_id));
                        }
                        FeedNotice::Projects => projects_changed = true,
                        _ => {}
                    }
                }
            }
            "milestone" => {
                if let Ok(project_id) = resolve_milestone_project(service, event) {
                    roadmap_projects.insert(project_id);
                } else {
                    projects_changed = true;
                }
            }
            "workflow" | "workflow_run" => automation_changed = true,
            "binding" | "assignment" | "artifact" | "project_asset" => {
                if let Some(kind) = loose_entity_event_kind(service, event) {
                    match kind {
                        FeedNotice::Tasks {
                            project_id,
                            task_id,
                        } => {
                            task_keys.insert((project_id, task_id));
                        }
                        FeedNotice::Projects => projects_changed = true,
                        _ => {}
                    }
                }
            }
            _ => projects_changed = true,
        }
    }

    let mut kinds = Vec::new();
    if projects_changed {
        kinds.push(FeedNotice::Projects);
    }
    for (project_id, task_id) in task_keys {
        kinds.push(FeedNotice::Tasks {
            project_id,
            task_id,
        });
    }
    for project_id in roadmap_projects {
        kinds.push(FeedNotice::Roadmap { project_id });
    }
    if automation_changed {
        kinds.push(FeedNotice::Automation);
    }
    kinds
}

fn task_project_id(service: &ProductChangeFeedService, task_id: &TaskId) -> Option<ProjectId> {
    let entity = service
        .authority
        .client()
        .ok()?
        .products()
        .get_entity(ProductEntityKind::Task, task_id.as_str())
        .ok()?;
    match entity {
        lilia_contracts::ProductEntity::Task(task) => task.project_id,
        _ => None,
    }
}

fn conversation_event_kind(
    service: &ProductChangeFeedService,
    event: &ProductEvent,
) -> Option<FeedNotice> {
    let entity = service
        .authority
        .client()
        .ok()?
        .products()
        .get_entity(ProductEntityKind::Conversation, &event.entity_id)
        .ok()?;
    let conversation = match entity {
        lilia_contracts::ProductEntity::Conversation(value) => value,
        _ => return Some(FeedNotice::Projects),
    };
    Some(FeedNotice::Tasks {
        project_id: conversation.project_id,
        task_id: conversation.task_id,
    })
}

fn resolve_milestone_project(
    service: &ProductChangeFeedService,
    event: &ProductEvent,
) -> Result<ProjectId, DesktopApplicationError> {
    let entity = service
        .authority
        .client()?
        .products()
        .get_entity(ProductEntityKind::Milestone, &event.entity_id)?;
    match entity {
        lilia_contracts::ProductEntity::Milestone(milestone) => Ok(milestone.project_id),
        _ => Err(DesktopApplicationError::InvalidInput {
            field: "entity",
            message: "milestone event did not resolve to a milestone entity".to_owned(),
        }),
    }
}

fn loose_entity_event_kind(
    service: &ProductChangeFeedService,
    event: &ProductEvent,
) -> Option<FeedNotice> {
    let kind = match event.entity.as_str() {
        "binding" => ProductEntityKind::Binding,
        "assignment" => ProductEntityKind::Assignment,
        "artifact" => ProductEntityKind::Artifact,
        "project_asset" => ProductEntityKind::ProjectAsset,
        _ => return Some(FeedNotice::Projects),
    };
    let entity = service
        .authority
        .client()
        .ok()?
        .products()
        .get_entity(kind, &event.entity_id)
        .ok()?;
    match entity {
        lilia_contracts::ProductEntity::Binding(binding) => Some(FeedNotice::Tasks {
            project_id: None,
            task_id: Some(binding.task_id),
        }),
        lilia_contracts::ProductEntity::Assignment(assignment) => Some(FeedNotice::Tasks {
            project_id: None,
            task_id: Some(assignment.task_id),
        }),
        lilia_contracts::ProductEntity::Artifact(artifact) => Some(FeedNotice::Tasks {
            project_id: None,
            task_id: Some(artifact.task_id),
        }),
        lilia_contracts::ProductEntity::ProjectAsset(_) => Some(FeedNotice::Projects),
        _ => Some(FeedNotice::Projects),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use lilia_contracts::{ProductCommandMeta, Project, ProjectId};
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::application::{
        DesktopApplicationConfig, DesktopHost, DesktopHostAction, DesktopHostContext,
        DesktopHostError, DesktopHostResult, DesktopProjectCreate,
    };

    #[derive(Default)]
    struct TestHost;

    impl DesktopHost for TestHost {
        fn execute(
            &self,
            _context: &DesktopHostContext,
            _action: DesktopHostAction,
        ) -> Result<DesktopHostResult, DesktopHostError> {
            Ok(DesktopHostResult::Completed)
        }
    }

    fn temp_app() -> (TempDir, DesktopApplication) {
        let dir = TempDir::new().unwrap();
        let config =
            DesktopApplicationConfig::new(dir.path(), format!("feed-{}", Uuid::new_v4())).unwrap();
        let app = DesktopApplication::bootstrap(config, Arc::new(TestHost)).unwrap();
        (dir, app)
    }

    #[test]
    fn poll_emits_invalidation_for_external_product_writes() {
        let (_dir, app) = temp_app();
        let _ = app.seed_product_change_feed_cursor().unwrap();
        let subscription = app.subscribe_events();

        let project =
            Project::new(ProjectId::new("external-project").unwrap(), "External").unwrap();
        let meta = ProductCommandMeta {
            command_id: format!("cmd-{}", Uuid::new_v4()),
            idempotency_key: lilia_contracts::IdempotencyKey::new(format!(
                "idem-{}",
                Uuid::new_v4()
            ))
            .unwrap(),
            expected_revision: None,
        };
        app.authority()
            .client()
            .unwrap()
            .create_product_entity(
                &meta,
                lilia_contracts::ProductEntity::Project(project),
                "created",
            )
            .unwrap();

        let published = app.poll_product_change_feed().unwrap();
        assert!(!published.is_empty());
        assert!(published.iter().any(|event| event.is::<ProjectsChanged>()));

        let received = subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("subscriber should receive feed event");
        assert!(received.is::<ProjectsChanged>());
    }

    #[test]
    fn seed_skips_historical_events_until_new_writes_arrive() {
        let (_dir, app) = temp_app();
        app.create_project(DesktopProjectCreate::new("Seeded"))
            .unwrap();
        let cursor = app.seed_product_change_feed_cursor().unwrap();
        assert!(cursor > 0);
        assert!(app.poll_product_change_feed().unwrap().is_empty());
    }

    #[test]
    fn background_feed_can_stop_and_restart_without_a_stale_running_flag() {
        let (_dir, app) = temp_app();

        app.start_product_change_feed_with_interval(Duration::from_millis(100))
            .unwrap();
        assert!(
            app.product_change_feed_service()
                .state
                .running
                .load(Ordering::SeqCst)
        );
        app.stop_product_change_feed();
        assert!(
            !app.product_change_feed_service()
                .state
                .running
                .load(Ordering::SeqCst)
        );

        app.start_product_change_feed_with_interval(Duration::from_millis(100))
            .unwrap();
        assert!(
            app.product_change_feed_service()
                .state
                .running
                .load(Ordering::SeqCst)
        );
        app.stop_product_change_feed();
    }
}
