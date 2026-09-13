use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use lilia_feature_memory::MemoryFeature;
use lilia_feature_roadmap::RoadmapFeature;
use lilia_kernel::Kernel;
use lilia_service::ServiceAuthority;
use nana_ui_platform::WindowId;

use crate::application::*;
use crate::runtime_shell::{ShellProjectPage, empty_snapshot};
use crate::shell_service::{WorkspaceSessionFeature, WorkspaceSessions};
use crate::ui_module::projection::{MemoryProjection, Projection, RoadmapProjection};
use crate::ui_module::{UiModule, UiModuleContext};

use super::memory::{MemoryMessage, MemoryModule};
use super::roadmap::{RoadmapMessage, RoadmapModule};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct NoopHost;
impl DesktopHost for NoopHost {
    fn execute(
        &self,
        _: &DesktopHostContext,
        _: DesktopHostAction,
    ) -> Result<DesktopHostResult, DesktopHostError> {
        Ok(DesktopHostResult::Completed)
    }
}

fn fixture() -> (
    tempfile::TempDir,
    DesktopApplication,
    Kernel,
    Arc<WorkspaceSessions>,
) {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let directory = tempfile::tempdir().unwrap();
    let authority = ServiceAuthority::bootstrap_with_home(directory.path()).unwrap();
    let app = DesktopApplication::from_authority(
        DesktopApplicationConfig::new(directory.path(), format!("liliacode.project-refresh.{id}"))
            .unwrap(),
        authority,
        Arc::new(NoopHost),
    )
    .unwrap();
    let sessions = Arc::new(WorkspaceSessions::new());
    let kernel = Kernel::new();
    kernel
        .mount(Arc::new(MemoryFeature::new(app.memory_service())))
        .unwrap();
    kernel
        .mount(Arc::new(RoadmapFeature::new(app.roadmap_service())))
        .unwrap();
    kernel
        .mount(Arc::new(WorkspaceSessionFeature::new(sessions.clone())))
        .unwrap();
    (directory, app, kernel, sessions)
}

fn install(
    app: &DesktopApplication,
    sessions: &WorkspaceSessions,
    window: WindowId,
    project: &lilia_contracts::ProjectId,
) {
    let session = app.create_workspace_session(
        DesktopWorkspaceSessionId::new(format!("window-{}", window.0)).unwrap(),
    );
    session
        .execute(DesktopCommand::SelectProject(project.clone()))
        .unwrap();
    sessions.install(window, session);
}

fn memory_input(
    memory: Option<&DesktopMemory>,
    project: Option<&lilia_contracts::ProjectId>,
) -> MemoryUpsertInput {
    MemoryUpsertInput {
        id: memory.map(|memory| memory.id.clone()),
        scope: if project.is_some() {
            MemoryScope::Project
        } else {
            MemoryScope::User
        },
        project_id: project.map(ToString::to_string),
        title: "Remember".to_owned(),
        body: "Saved body".to_owned(),
        tags: Vec::new(),
        enabled: true,
        source_task_id: None,
        expected_updated_at: memory.map(|memory| memory.updated_at),
    }
}

fn project_roadmap(
    module: &RoadmapModule,
    cx: &UiModuleContext<'_>,
    snapshot: &mut crate::runtime_shell::PrimaryShellSnapshot,
) {
    module.project_fields(cx, RoadmapProjection::fields(snapshot));
}

fn project_memory(
    module: &MemoryModule,
    cx: &UiModuleContext<'_>,
    snapshot: &mut crate::runtime_shell::PrimaryShellSnapshot,
) {
    module.project_fields(cx, MemoryProjection::fields(snapshot));
}

#[test]
fn project_refresh_roadmap_keeps_edits_during_link_and_other_window_changes() {
    let (_directory, app, kernel, sessions) = fixture();
    let project = app
        .create_project(DesktopProjectCreate::new("Project"))
        .unwrap();
    let task = app
        .create_task(DesktopTaskCreate::new(Some(project.id.clone()), "Task"))
        .unwrap();
    let milestone = app.create_milestone(&project.id, "Saved title").unwrap();
    install(&app, &sessions, WindowId::PRIMARY, &project.id);
    install(&app, &sessions, WindowId(2), &project.id);
    let cx =
        UiModuleContext::new(&kernel, WindowId::PRIMARY).showing(Some(ShellProjectPage::Roadmap));
    let other_cx =
        UiModuleContext::new(&kernel, WindowId(2)).showing(Some(ShellProjectPage::Roadmap));
    let mut module = RoadmapModule::default();
    let mut other = RoadmapModule::default();
    module.reduce(RoadmapMessage::Refresh, &cx);
    other.reduce(RoadmapMessage::Refresh, &other_cx);
    module.reduce(RoadmapMessage::TitleChanged("Draft title".to_owned()), &cx);
    module.reduce(
        RoadmapMessage::DescriptionChanged("Line one\nLine two".to_owned()),
        &cx,
    );
    module.reduce(RoadmapMessage::DueDateChanged("2026-10-08".to_owned()), &cx);
    module.reduce(RoadmapMessage::ToggleTask(task.id.to_string()), &cx);
    assert_eq!(app.project_roadmap(&project.id).unwrap().links.len(), 1);
    assert_eq!(
        app.project_roadmap(&project.id).unwrap().milestones[0].title,
        "Saved title"
    );
    let events = app.subscribe_events();
    other.reduce(RoadmapMessage::CycleStatus, &other_cx);
    while let Ok(event) = events.try_recv() {
        module.invalidate(event.envelope(), &cx);
    }
    module.reduce(RoadmapMessage::TitleChanged("Draft title".to_owned()), &cx);
    module.reduce(
        RoadmapMessage::DescriptionChanged("Line one\nLine two".to_owned()),
        &cx,
    );
    module.reduce(RoadmapMessage::DueDateChanged("2026-10-08".to_owned()), &cx);
    module.restore_selection(Some(milestone.id.clone()));
    let mut snapshot = empty_snapshot();
    project_roadmap(&module, &cx, &mut snapshot);
    assert_eq!(snapshot.roadmap.title, "Saved title");
    module.reduce(RoadmapMessage::TitleChanged("Draft title".to_owned()), &cx);
    module.reduce(
        RoadmapMessage::DescriptionChanged("Line one\nLine two".to_owned()),
        &cx,
    );
    module.reduce(RoadmapMessage::DueDateChanged("2026-10-08".to_owned()), &cx);
    module.reduce(RoadmapMessage::Save, &cx);
    assert!(module.error().is_none());
    let saved = app.project_roadmap(&project.id).unwrap();
    assert_eq!(saved.milestones[0].title, "Draft title");
    assert_eq!(saved.milestones[0].description, "Line one\nLine two");
    assert_eq!(
        saved.milestones[0].status,
        other.roadmap().milestones[0].status
    );
    module.reduce(
        RoadmapMessage::TitleChanged("Discard on selection".to_owned()),
        &cx,
    );
    let next = app.create_milestone(&project.id, "Next").unwrap();
    module.reduce(RoadmapMessage::Refresh, &cx);
    module.reduce(RoadmapMessage::Select(next.id), &cx);
    assert_eq!(module.title(), "Next");
}

#[test]
fn project_refresh_memory_popup_restoration_keeps_existing_and_new_drafts() {
    let (_directory, app, kernel, sessions) = fixture();
    let project = app
        .create_project(DesktopProjectCreate::new("Project"))
        .unwrap();
    let memory = app
        .save_memory(memory_input(None, Some(&project.id)))
        .unwrap();
    install(&app, &sessions, WindowId(2), &project.id);
    let cx = UiModuleContext::new(&kernel, WindowId(2)).showing(Some(ShellProjectPage::Memory));
    let mut module = MemoryModule::default();
    module.reduce(MemoryMessage::Refresh, &cx);
    module.reduce(
        MemoryMessage::BodyReplaced("Unsent\nTwo lines".to_owned()),
        &cx,
    );
    module.reduce(MemoryMessage::Save, &cx);
    assert_eq!(
        app.memory(&memory.id).unwrap().unwrap().body,
        "Unsent\nTwo lines"
    );
    module.reduce(MemoryMessage::New, &cx);
    module.reduce(MemoryMessage::TitleChanged("New draft".to_owned()), &cx);
    module.reduce(
        MemoryMessage::BodyReplaced("Not yet stored".to_owned()),
        &cx,
    );
    let mut snapshot = empty_snapshot();
    project_memory(&module, &cx, &mut snapshot);
    assert_eq!(snapshot.memory.title, "New draft");
    assert_eq!(snapshot.memory.body, "Not yet stored");
    assert_eq!(snapshot.memory.selected, None);
    module.reduce(MemoryMessage::Select(memory.id), &cx);
    project_memory(&module, &cx, &mut snapshot);
    assert_eq!(snapshot.memory.body, "Unsent\nTwo lines");
}

#[test]
fn project_refresh_memory_scope_move_invalidates_previous_and_current_visibility() {
    let (_directory, app, kernel, sessions) = fixture();
    let first = app
        .create_project(DesktopProjectCreate::new("First"))
        .unwrap();
    let second = app
        .create_project(DesktopProjectCreate::new("Second"))
        .unwrap();
    install(&app, &sessions, WindowId::PRIMARY, &first.id);
    install(&app, &sessions, WindowId(2), &second.id);
    let first_cx =
        UiModuleContext::new(&kernel, WindowId::PRIMARY).showing(Some(ShellProjectPage::Memory));
    let second_cx =
        UiModuleContext::new(&kernel, WindowId(2)).showing(Some(ShellProjectPage::Memory));
    let mut left = MemoryModule::default();
    let mut right = MemoryModule::default();
    let mut memory = app.save_memory(memory_input(None, None)).unwrap();
    left.reduce(MemoryMessage::Refresh, &first_cx);
    right.reduce(MemoryMessage::Refresh, &second_cx);
    let events = app.subscribe_events();
    for (project, expected_counts) in [
        (Some(&first.id), (1, 0)),
        (Some(&second.id), (0, 1)),
        (Some(&second.id), (0, 1)),
        (None, (1, 1)),
    ] {
        memory = app
            .save_memory(memory_input(Some(&memory), project))
            .unwrap();
        while let Ok(event) = events.try_recv() {
            if event.downcast::<MemoryChanged>().is_some() {
                left.invalidate(event.envelope(), &first_cx);
                right.invalidate(event.envelope(), &second_cx);
            }
        }
        let mut left_snapshot = empty_snapshot();
        let mut right_snapshot = empty_snapshot();
        project_memory(&left, &first_cx, &mut left_snapshot);
        project_memory(&right, &second_cx, &mut right_snapshot);
        assert_eq!(
            (
                left_snapshot.memory.cards.len(),
                right_snapshot.memory.cards.len()
            ),
            expected_counts
        );
        assert_eq!(
            app.list_memories(Some(&first.id)).unwrap().len(),
            expected_counts.0
        );
        assert_eq!(
            app.list_memories(Some(&second.id)).unwrap().len(),
            expected_counts.1
        );
    }
}

#[test]
fn roadmap_completion_projects_authoritative_task_status_without_changing_selection() {
    let (_directory, app, kernel, sessions) = fixture();
    let project = app
        .create_project(DesktopProjectCreate::new("Progress"))
        .unwrap();
    let linked = app
        .create_task(DesktopTaskCreate::new(Some(project.id.clone()), "Linked"))
        .unwrap();
    let unrelated = app
        .create_task(DesktopTaskCreate::new(
            Some(project.id.clone()),
            "Unrelated",
        ))
        .unwrap();
    let milestone = app.create_milestone(&project.id, "Ship").unwrap();
    app.set_milestone_tasks(&project.id, &milestone.id, vec![linked.id.to_string()])
        .unwrap();
    install(&app, &sessions, WindowId::PRIMARY, &project.id);
    let cx =
        UiModuleContext::new(&kernel, WindowId::PRIMARY).showing(Some(ShellProjectPage::Roadmap));
    let workspace = cx.workspace().unwrap();
    workspace
        .execute(DesktopCommand::OpenWorkspaceItem {
            pane_id: PaneId::new("primary").unwrap(),
            item: app.task_workspace_item(&unrelated.id).unwrap(),
        })
        .unwrap();
    let mut module = RoadmapModule::default();
    module.reduce(RoadmapMessage::Refresh, &cx);
    for completed in [false, true, false] {
        app.set_task_completed(&linked.id, completed).unwrap();
        workspace.execute(DesktopCommand::RefreshWorkspace).unwrap();
        module.reduce(RoadmapMessage::Refresh, &cx);
        let mut snapshot = empty_snapshot();
        project_roadmap(&module, &cx, &mut snapshot);
        assert_eq!(snapshot.roadmap.cards[0].id, milestone.id);
        assert_eq!(
            workspace.snapshot().unwrap().selected_task,
            Some(unrelated.id.clone())
        );
        assert_eq!(app.project_roadmap(&project.id).unwrap().links.len(), 1);
    }
    workspace
        .execute(DesktopCommand::OpenWorkspaceItem {
            pane_id: PaneId::new("primary").unwrap(),
            item: app.task_workspace_item(&linked.id).unwrap(),
        })
        .unwrap();
    app.set_task_completed(&linked.id, true).unwrap();
    workspace.execute(DesktopCommand::RefreshWorkspace).unwrap();
    module.reduce(RoadmapMessage::Refresh, &cx);
    let mut snapshot = empty_snapshot();
    project_roadmap(&module, &cx, &mut snapshot);
    assert_eq!(snapshot.roadmap.cards[0].id, milestone.id);
    assert_eq!(workspace.snapshot().unwrap().selected_task, Some(linked.id));
}

#[test]
fn memory_toggle_enabled_persists_immediately_and_new_user_memory_saves() {
    let (_directory, app, kernel, sessions) = fixture();
    let project = app
        .create_project(DesktopProjectCreate::new("Memory forms"))
        .unwrap();
    let memory = app
        .save_memory(memory_input(None, Some(&project.id)))
        .unwrap();
    install(&app, &sessions, WindowId(2), &project.id);
    let cx = UiModuleContext::new(&kernel, WindowId(2)).showing(Some(ShellProjectPage::Memory));
    let mut module = MemoryModule::default();
    module.reduce(MemoryMessage::Refresh, &cx);
    module.reduce(MemoryMessage::Select(memory.id.clone()), &cx);
    module.reduce(MemoryMessage::ToggleEnabled, &cx);
    assert!(!app.memory(&memory.id).unwrap().unwrap().enabled);

    module.reduce(MemoryMessage::New, &cx);
    module.reduce(MemoryMessage::ToggleScope, &cx);
    module.reduce(
        MemoryMessage::TitleChanged("Disabled user draft".into()),
        &cx,
    );
    module.reduce(MemoryMessage::BodyReplaced("Draft body".into()), &cx);
    module.reduce(MemoryMessage::Save, &cx);
    let saved = app.memory(module.selected().unwrap()).unwrap().unwrap();
    assert_eq!(saved.scope, MemoryScope::User);
    assert!(saved.enabled);
    assert!(saved.project_id.is_none());
    module.reduce(MemoryMessage::ToggleEnabled, &cx);
    assert!(
        !app.memory(module.selected().unwrap())
            .unwrap()
            .unwrap()
            .enabled
    );
}

#[test]
fn memory_select_does_not_mutate_other_records() {
    let (_directory, app, kernel, sessions) = fixture();
    let project = app
        .create_project(DesktopProjectCreate::new("Memory cards"))
        .unwrap();
    let first = app
        .save_memory(memory_input(None, Some(&project.id)))
        .unwrap();
    let second = app
        .save_memory(memory_input(None, Some(&project.id)))
        .unwrap();
    install(&app, &sessions, WindowId(2), &project.id);
    let cx = UiModuleContext::new(&kernel, WindowId(2)).showing(Some(ShellProjectPage::Memory));
    let mut module = MemoryModule::default();
    module.reduce(MemoryMessage::Refresh, &cx);
    module.reduce(MemoryMessage::Select(first.id.clone()), &cx);
    module.reduce(
        MemoryMessage::BodyReplaced("Unsaved first draft".into()),
        &cx,
    );
    module.reduce(MemoryMessage::Save, &cx);
    module.reduce(MemoryMessage::Select(second.id.clone()), &cx);
    module.reduce(MemoryMessage::ToggleEnabled, &cx);
    assert!(!app.memory(&second.id).unwrap().unwrap().enabled);
    assert_eq!(module.selected(), Some(second.id.as_str()));
    module.reduce(MemoryMessage::Delete, &cx);
    assert!(app.memory(&second.id).unwrap().is_none());
    assert_eq!(
        app.memory(&first.id).unwrap().unwrap().body,
        "Unsaved first draft"
    );
}

#[test]
fn memory_toggle_preserves_stale_draft_conflict_after_external_update() {
    let (_directory, app, kernel, sessions) = fixture();
    let project = app
        .create_project(DesktopProjectCreate::new("Concurrent memory"))
        .unwrap();
    let original = app
        .save_memory(memory_input(None, Some(&project.id)))
        .unwrap();
    install(&app, &sessions, WindowId(2), &project.id);
    let cx = UiModuleContext::new(&kernel, WindowId(2)).showing(Some(ShellProjectPage::Memory));
    let mut module = MemoryModule::default();
    module.reduce(MemoryMessage::Refresh, &cx);
    module.reduce(MemoryMessage::Select(original.id.clone()), &cx);
    module.reduce(
        MemoryMessage::BodyReplaced("Unsaved local draft".into()),
        &cx,
    );
    let events = app.subscribe_events();
    let mut external = memory_input(Some(&original), Some(&project.id));
    external.body = "Saved in another window".into();
    let external = app.save_memory(external).unwrap();
    while let Ok(event) = events.try_recv() {
        module.invalidate(event.envelope(), &cx);
    }
    module.reduce(MemoryMessage::Save, &cx);
    assert!(
        module.error().is_some(),
        "stale draft must remain conflicted after an external save"
    );
    assert_eq!(
        app.memory(&original.id).unwrap().unwrap().body,
        external.body
    );
    assert_eq!(module.body().text(), "Unsaved local draft");

    module.reduce(MemoryMessage::Refresh, &cx);
    module.reduce(MemoryMessage::Select(original.id.clone()), &cx);
    module.reduce(
        MemoryMessage::BodyReplaced("Edited from current revision".into()),
        &cx,
    );
    module.reduce(MemoryMessage::Save, &cx);
    assert!(module.error().is_none());
    let saved = app.memory(&original.id).unwrap().unwrap();
    assert_eq!(saved.body, "Edited from current revision");
    assert!(saved.enabled);
}

#[test]
fn memory_save_creates_from_dirty_draft_after_selected_row_vanishes() {
    let (_directory, app, kernel, sessions) = fixture();
    let project = app
        .create_project(DesktopProjectCreate::new("Deleted memory"))
        .unwrap();
    let original = app
        .save_memory(memory_input(None, Some(&project.id)))
        .unwrap();
    install(&app, &sessions, WindowId(2), &project.id);
    let cx = UiModuleContext::new(&kernel, WindowId(2)).showing(Some(ShellProjectPage::Memory));
    let mut module = MemoryModule::default();
    module.reduce(MemoryMessage::Refresh, &cx);
    module.reduce(MemoryMessage::Select(original.id.clone()), &cx);
    module.reduce(MemoryMessage::TitleChanged("Kept title".into()), &cx);
    module.reduce(
        MemoryMessage::BodyReplaced("Kept body".into()),
        &cx,
    );
    let events = app.subscribe_events();
    app.delete_memory(&original.id, Some(original.updated_at))
        .unwrap();
    while let Ok(event) = events.try_recv() {
        module.invalidate(event.envelope(), &cx);
    }
    assert_eq!(module.selected(), None);
    assert_eq!(module.title(), "Kept title");
    assert_eq!(module.body().text(), "Kept body");
    module.reduce(MemoryMessage::Save, &cx);
    assert!(module.error().is_none());
    let created = app.memory(module.selected().unwrap()).unwrap().unwrap();
    assert_ne!(created.id, original.id);
    assert_eq!(created.title, "Kept title");
    assert_eq!(created.body, "Kept body");
}

#[test]
fn memory_clean_editor_adopts_external_body_and_saves() {
    let (_directory, app, kernel, sessions) = fixture();
    let project = app
        .create_project(DesktopProjectCreate::new("Remote memory"))
        .unwrap();
    let original = app
        .save_memory(memory_input(None, Some(&project.id)))
        .unwrap();
    install(&app, &sessions, WindowId(2), &project.id);
    let cx = UiModuleContext::new(&kernel, WindowId(2)).showing(Some(ShellProjectPage::Memory));
    let mut module = MemoryModule::default();
    module.reduce(MemoryMessage::Refresh, &cx);
    module.reduce(MemoryMessage::Select(original.id.clone()), &cx);
    assert_eq!(module.body().text(), "Saved body");
    let events = app.subscribe_events();
    let mut external = memory_input(Some(&original), Some(&project.id));
    external.body = "Saved in another window".into();
    let external = app.save_memory(external).unwrap();
    while let Ok(event) = events.try_recv() {
        module.invalidate(event.envelope(), &cx);
    }
    assert_eq!(module.body().text(), "Saved in another window");
    module.reduce(MemoryMessage::Save, &cx);
    assert!(module.error().is_none());
    assert_eq!(
        app.memory(&original.id).unwrap().unwrap().body,
        external.body
    );
}

#[test]
fn memory_settings_and_enable_refresh_keep_local_draft() {
    let (_directory, app, kernel, sessions) = fixture();
    let project = app
        .create_project(DesktopProjectCreate::new("Draft memory"))
        .unwrap();
    let task = app
        .create_task(DesktopTaskCreate::new(Some(project.id.clone()), "Session"))
        .unwrap();
    let memory = app
        .save_memory(memory_input(None, Some(&project.id)))
        .unwrap();
    install(&app, &sessions, WindowId(2), &project.id);
    let cx = UiModuleContext::new(&kernel, WindowId(2)).showing(Some(ShellProjectPage::Memory));
    cx.workspace()
        .unwrap()
        .execute(DesktopCommand::OpenWorkspaceItem {
            pane_id: PaneId::new("primary").unwrap(),
            item: app.task_workspace_item(&task.id).unwrap(),
        })
        .unwrap();
    let mut module = MemoryModule::default();
    module.reduce(MemoryMessage::Refresh, &cx);
    module.reduce(MemoryMessage::Select(memory.id.clone()), &cx);
    module.reduce(MemoryMessage::BodyReplaced("Local draft".into()), &cx);
    let events = app.subscribe_events();
    module.reduce(MemoryMessage::ToggleGlobal, &cx);
    module.reduce(MemoryMessage::ToggleBaseline, &cx);
    module.reduce(MemoryMessage::CooldownChanged("4".into()), &cx);
    module.reduce(MemoryMessage::ToggleTaskInjection, &cx);
    module.reduce(MemoryMessage::ToggleEnabled, &cx);
    while let Ok(event) = events.try_recv() {
        module.invalidate(event.envelope(), &cx);
    }
    assert_eq!(module.body().text(), "Local draft");
    assert!(!module.settings().enabled);
    assert!(!module.settings().baseline_injection_enabled);
    assert_eq!(module.settings().cooldown_turns, 4);
    assert!(!module.injection().unwrap().enabled);
    assert!(!app.memory(&memory.id).unwrap().unwrap().enabled);
    module.reduce(MemoryMessage::Save, &cx);
    assert!(module.error().is_none());
    let saved = app.memory(&memory.id).unwrap().unwrap();
    assert_eq!(saved.body, "Local draft");
    assert!(!saved.enabled);
}

#[test]
fn memory_save_keeps_source_task_id() {
    let (_directory, app, kernel, sessions) = fixture();
    let project = app
        .create_project(DesktopProjectCreate::new("Provenance"))
        .unwrap();
    let task = app
        .create_task(DesktopTaskCreate::new(Some(project.id.clone()), "Source"))
        .unwrap();
    let mut input = memory_input(None, Some(&project.id));
    input.source_task_id = Some(task.id.as_str().to_owned());
    let memory = app.save_memory(input).unwrap();
    assert_eq!(memory.source_task_id.as_deref(), Some(task.id.as_str()));
    install(&app, &sessions, WindowId(2), &project.id);
    let cx = UiModuleContext::new(&kernel, WindowId(2)).showing(Some(ShellProjectPage::Memory));
    let mut module = MemoryModule::default();
    module.reduce(MemoryMessage::Refresh, &cx);
    module.reduce(MemoryMessage::Select(memory.id.clone()), &cx);
    module.reduce(MemoryMessage::BodyReplaced("Edited body".into()), &cx);
    module.reduce(MemoryMessage::Save, &cx);
    assert!(module.error().is_none());
    let saved = app.memory(&memory.id).unwrap().unwrap();
    assert_eq!(saved.body, "Edited body");
    assert_eq!(saved.source_task_id.as_deref(), Some(task.id.as_str()));
    module.reduce(MemoryMessage::New, &cx);
    module.reduce(MemoryMessage::TitleChanged("Fresh".into()), &cx);
    module.reduce(MemoryMessage::BodyReplaced("No provenance".into()), &cx);
    module.reduce(MemoryMessage::Save, &cx);
    let created = app.memory(module.selected().unwrap()).unwrap().unwrap();
    assert!(created.source_task_id.is_none());
}

#[test]
fn project_refresh_roadmap_keeps_dirty_editor_across_links_and_events() {
    let (_directory, app, kernel, sessions) = fixture();
    let project = app
        .create_project(DesktopProjectCreate::new("Roadmap draft"))
        .unwrap();
    let task = app
        .create_task(DesktopTaskCreate::new(Some(project.id.clone()), "Linked"))
        .unwrap();
    let milestone = app.create_milestone(&project.id, "Saved title").unwrap();
    install(&app, &sessions, WindowId::PRIMARY, &project.id);
    install(&app, &sessions, WindowId(2), &project.id);
    let cx =
        UiModuleContext::new(&kernel, WindowId::PRIMARY).showing(Some(ShellProjectPage::Roadmap));
    let other_cx =
        UiModuleContext::new(&kernel, WindowId(2)).showing(Some(ShellProjectPage::Roadmap));
    let mut module = RoadmapModule::default();
    let mut other = RoadmapModule::default();
    module.reduce(RoadmapMessage::Refresh, &cx);
    other.reduce(RoadmapMessage::Refresh, &other_cx);
    module.reduce(RoadmapMessage::Select(milestone.id.clone()), &cx);
    module.reduce(RoadmapMessage::TitleChanged("Draft title".to_owned()), &cx);
    module.reduce(
        RoadmapMessage::DescriptionChanged("Line one\nLine two".to_owned()),
        &cx,
    );
    module.reduce(RoadmapMessage::DueDateChanged("2026-10-08".to_owned()), &cx);
    module.reduce(RoadmapMessage::ToggleTask(task.id.to_string()), &cx);
    assert_eq!(module.title(), "Draft title");
    assert_eq!(app.project_roadmap(&project.id).unwrap().links.len(), 1);
    assert_eq!(
        app.project_roadmap(&project.id).unwrap().milestones[0].title,
        "Saved title"
    );
    let events = app.subscribe_events();
    other.reduce(RoadmapMessage::CycleStatus, &other_cx);
    while let Ok(event) = events.try_recv() {
        module.invalidate(event.envelope(), &cx);
    }
    assert_eq!(module.title(), "Draft title");
    let mut snapshot = empty_snapshot();
    project_roadmap(&module, &cx, &mut snapshot);
    assert_eq!(snapshot.roadmap.title, "Draft title");
    assert_eq!(snapshot.roadmap.description, "Line one\nLine two");
    assert_eq!(snapshot.roadmap.due_date, "2026-10-08");
}

#[test]
fn roadmap_clean_editor_adopts_external_title_and_dirty_draft_survives_delete() {
    let (_directory, app, kernel, sessions) = fixture();
    let project = app
        .create_project(DesktopProjectCreate::new("Roadmap remote"))
        .unwrap();
    let first = app.create_milestone(&project.id, "Saved title").unwrap();
    let second = app.create_milestone(&project.id, "Keep").unwrap();
    install(&app, &sessions, WindowId::PRIMARY, &project.id);
    let cx =
        UiModuleContext::new(&kernel, WindowId::PRIMARY).showing(Some(ShellProjectPage::Roadmap));
    let mut module = RoadmapModule::default();
    module.reduce(RoadmapMessage::Refresh, &cx);
    module.reduce(RoadmapMessage::Select(first.id.clone()), &cx);
    assert_eq!(module.title(), "Saved title");
    let events = app.subscribe_events();
    app.update_milestone(
        &project.id,
        &first.id,
        MilestoneUpdatePatch {
            title: Some("Other window".to_owned()),
            ..MilestoneUpdatePatch::default()
        },
    )
    .unwrap();
    while let Ok(event) = events.try_recv() {
        module.invalidate(event.envelope(), &cx);
    }
    assert_eq!(module.title(), "Other window");
    module.reduce(RoadmapMessage::Save, &cx);
    assert!(module.error().is_none());
    assert_eq!(
        app.project_roadmap(&project.id).unwrap().milestones[0].title,
        "Other window"
    );

    module.reduce(
        RoadmapMessage::TitleChanged("Local draft".to_owned()),
        &cx,
    );
    app.update_milestone(
        &project.id,
        &first.id,
        MilestoneUpdatePatch {
            title: Some("Clobber me".to_owned()),
            ..MilestoneUpdatePatch::default()
        },
    )
    .unwrap();
    while let Ok(event) = events.try_recv() {
        module.invalidate(event.envelope(), &cx);
    }
    assert_eq!(module.title(), "Local draft");
    assert_eq!(
        app.project_roadmap(&project.id).unwrap().milestones[0].title,
        "Clobber me"
    );

    app.delete_milestone(&project.id, &first.id).unwrap();
    while let Ok(event) = events.try_recv() {
        module.invalidate(event.envelope(), &cx);
    }
    assert_eq!(module.selected(), None);
    assert_eq!(module.title(), "Local draft");
    assert_eq!(
        app.project_roadmap(&project.id).unwrap().milestones[0].title,
        "Keep"
    );
    module.reduce(RoadmapMessage::Save, &cx);
    assert!(module.error().is_none());
    assert_eq!(module.title(), "Local draft");
    let titles = app
        .project_roadmap(&project.id)
        .unwrap()
        .milestones
        .into_iter()
        .map(|milestone| milestone.title)
        .collect::<Vec<_>>();
    assert!(titles.contains(&"Keep".to_owned()));
    assert!(titles.contains(&"Local draft".to_owned()));
    module.reduce(RoadmapMessage::Save, &cx);
    assert!(module.error().is_none());
    let titles = app
        .project_roadmap(&project.id)
        .unwrap()
        .milestones
        .into_iter()
        .map(|milestone| milestone.title)
        .collect::<Vec<_>>();
    assert_eq!(
        titles
            .iter()
            .filter(|title| title.as_str() == "Local draft")
            .count(),
        1
    );
    let _ = second;
}

#[test]
fn roadmap_save_adopts_loaded_snapshot_so_later_remote_fields_are_not_clobbered() {
    let (_directory, app, kernel, sessions) = fixture();
    let project = app
        .create_project(DesktopProjectCreate::new("Roadmap save"))
        .unwrap();
    let milestone = app.create_milestone(&project.id, "Saved title").unwrap();
    install(&app, &sessions, WindowId::PRIMARY, &project.id);
    let cx =
        UiModuleContext::new(&kernel, WindowId::PRIMARY).showing(Some(ShellProjectPage::Roadmap));
    let mut module = RoadmapModule::default();
    module.reduce(RoadmapMessage::Refresh, &cx);
    module.reduce(RoadmapMessage::Select(milestone.id.clone()), &cx);
    module.reduce(RoadmapMessage::TitleChanged("Mine".to_owned()), &cx);
    module.reduce(RoadmapMessage::Save, &cx);
    assert!(module.error().is_none());
    let events = app.subscribe_events();
    app.update_milestone(
        &project.id,
        &milestone.id,
        MilestoneUpdatePatch {
            description: Some("Remote description".to_owned()),
            ..MilestoneUpdatePatch::default()
        },
    )
    .unwrap();
    while let Ok(event) = events.try_recv() {
        module.invalidate(event.envelope(), &cx);
    }
    let mut snapshot = empty_snapshot();
    project_roadmap(&module, &cx, &mut snapshot);
    assert_eq!(snapshot.roadmap.title, "Mine");
    assert_eq!(snapshot.roadmap.description, "Remote description");
    module.reduce(RoadmapMessage::Save, &cx);
    assert!(module.error().is_none());
    let saved = app.project_roadmap(&project.id).unwrap();
    assert_eq!(saved.milestones[0].title, "Mine");
    assert_eq!(saved.milestones[0].description, "Remote description");
}
