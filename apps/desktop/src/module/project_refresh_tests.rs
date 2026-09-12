use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use lilia_kernel::Kernel;
use lilia_service::ServiceAuthority;
use nana_ui_platform::WindowId;

use crate::application::*;
use crate::runtime_shell::{empty_snapshot, ShellProjectPage};
use crate::shell_service::{ApplicationFeature, WorkspaceSessionFeature, WorkspaceSessions};
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
        .mount(Arc::new(ApplicationFeature::new(app.clone())))
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
    module.restore_selection(Some(milestone.id.clone()));
    let mut snapshot = empty_snapshot();
    module.project(&cx, &mut snapshot);
    assert_eq!(snapshot.milestone_title, "Draft title");
    assert_eq!(snapshot.milestone_description, "Line one\nLine two");
    assert_eq!(snapshot.milestone_due_date, "2026-10-08");
    assert!(snapshot.roadmap_tasks.iter().any(|(_, _, linked)| *linked));
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
    let events = app.subscribe_events();
    app.save_memory(memory_input(None, Some(&project.id)))
        .unwrap();
    while let Ok(event) = events.try_recv() {
        module.invalidate(event.envelope(), &cx);
    }
    module.reduce(MemoryMessage::Refresh, &cx);
    module.restore_selection(Some(memory.id.clone()));
    let mut snapshot = empty_snapshot();
    module.project(&cx, &mut snapshot);
    assert_eq!(snapshot.memory_body, "Unsent\nTwo lines");
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
    module.reduce(MemoryMessage::Refresh, &cx);
    module.restore_selection(None);
    module.project(&cx, &mut snapshot);
    assert_eq!(snapshot.memory_title, "New draft");
    assert_eq!(snapshot.memory_body, "Not yet stored");
    assert!(snapshot.memory_enabled.is_none());
    module.reduce(MemoryMessage::Select(memory.id), &cx);
    module.project(&cx, &mut snapshot);
    assert_eq!(snapshot.memory_body, "Unsent\nTwo lines");
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
    for (project, expected_scopes, expected_counts) in [
        (Some(&first.id), vec![None], (1, 0)),
        (
            Some(&second.id),
            vec![Some(second.id.clone()), Some(first.id.clone())],
            (0, 1),
        ),
        (Some(&second.id), vec![Some(second.id.clone())], (0, 1)),
        (None, vec![None], (1, 1)),
    ] {
        memory = app
            .save_memory(memory_input(Some(&memory), project))
            .unwrap();
        let mut scopes = Vec::new();
        while let Ok(event) = events.try_recv() {
            if let Some(changed) = event.downcast::<MemoryChanged>() {
                scopes.push(changed.project_id.clone());
                left.invalidate(event.envelope(), &first_cx);
                right.invalidate(event.envelope(), &second_cx);
            }
        }
        assert_eq!(scopes, expected_scopes);
        let mut left_snapshot = empty_snapshot();
        let mut right_snapshot = empty_snapshot();
        left.project(&first_cx, &mut left_snapshot);
        right.project(&second_cx, &mut right_snapshot);
        assert_eq!(
            (
                left_snapshot.memory_cards.len(),
                right_snapshot.memory_cards.len()
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
    for (completed, expected) in [
        (false, "0/1 已完成"),
        (true, "1/1 已完成"),
        (false, "0/1 已完成"),
    ] {
        app.set_task_completed(&linked.id, completed).unwrap();
        workspace.execute(DesktopCommand::RefreshWorkspace).unwrap();
        let mut snapshot = empty_snapshot();
        module.project(&cx, &mut snapshot);
        assert!(snapshot.roadmap_cards[0].status.contains(expected));
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
    let mut snapshot = empty_snapshot();
    module.project(&cx, &mut snapshot);
    assert!(snapshot.roadmap_cards[0].status.contains("1/1 已完成"));
    assert_eq!(workspace.snapshot().unwrap().selected_task, Some(linked.id));
}

#[test]
fn memory_editor_enabled_is_a_draft_and_new_disabled_memory_saves_as_disabled() {
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
    module.reduce(MemoryMessage::ToggleDraftEnabled, &cx);
    module.reduce(MemoryMessage::Refresh, &cx);
    assert!(app.memory(&memory.id).unwrap().unwrap().enabled);
    let mut snapshot = empty_snapshot();
    module.project(&cx, &mut snapshot);
    assert!(!snapshot.memory_draft_enabled);
    module.reduce(MemoryMessage::Save, &cx);
    assert!(!app.memory(&memory.id).unwrap().unwrap().enabled);

    module.reduce(MemoryMessage::NewInScope(MemoryScope::User), &cx);
    module.reduce(
        MemoryMessage::TitleChanged("Disabled user draft".into()),
        &cx,
    );
    module.reduce(MemoryMessage::BodyReplaced("Draft body".into()), &cx);
    module.reduce(MemoryMessage::ToggleDraftEnabled, &cx);
    module.reduce(MemoryMessage::Save, &cx);
    let saved = app.memory(module.selected().unwrap()).unwrap().unwrap();
    assert_eq!(saved.scope, MemoryScope::User);
    assert!(!saved.enabled);
    assert!(saved.project_id.is_none());
}

#[test]
fn memory_card_toggle_targets_its_record_and_keeps_other_editor_draft() {
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
    module.reduce(MemoryMessage::ToggleEntry(second.id.clone()), &cx);
    assert!(!app.memory(&second.id).unwrap().unwrap().enabled);
    assert_eq!(module.selected(), Some(first.id.as_str()));
    assert_eq!(module.body().text(), "Unsaved first draft");
    module.reduce(MemoryMessage::DeleteEntry(second.id.clone()), &cx);
    assert!(app.memory(&second.id).unwrap().is_none());
    assert_eq!(module.body().text(), "Unsaved first draft");
    module.reduce(MemoryMessage::Save, &cx);
    assert_eq!(
        app.memory(&first.id).unwrap().unwrap().body,
        "Unsaved first draft"
    );
}

#[test]
fn memory_toggle_preserves_stale_draft_conflict_after_external_update() {
    for legacy_toggle in [false, true] {
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
        let mut external = memory_input(Some(&original), Some(&project.id));
        external.body = "Saved in another window".into();
        let external = app.save_memory(external).unwrap();
        module.reduce(MemoryMessage::Refresh, &cx);
        assert_eq!(module.body().text(), "Unsaved local draft");
        module.reduce(
            if legacy_toggle {
                MemoryMessage::ToggleEnabled
            } else {
                MemoryMessage::ToggleEntry(original.id.clone())
            },
            &cx,
        );
        let toggled = app.memory(&original.id).unwrap().unwrap();
        assert!(!toggled.enabled);
        assert_eq!(toggled.body, external.body);
        assert_eq!(module.body().text(), "Unsaved local draft");
        module.reduce(MemoryMessage::Save, &cx);
        assert!(
            module.error().is_some(),
            "stale draft must remain conflicted after toggling"
        );
        assert_eq!(app.memory(&original.id).unwrap().unwrap(), toggled);
        assert_eq!(module.body().text(), "Unsaved local draft");

        module.reduce(MemoryMessage::Select(original.id.clone()), &cx);
        module.reduce(
            MemoryMessage::BodyReplaced("Edited from current revision".into()),
            &cx,
        );
        module.reduce(MemoryMessage::ToggleEntry(original.id.clone()), &cx);
        module.reduce(MemoryMessage::Save, &cx);
        assert!(module.error().is_none());
        let saved = app.memory(&original.id).unwrap().unwrap();
        assert_eq!(saved.body, "Edited from current revision");
        assert!(saved.enabled);
    }
}
