//! The roadmap page as a UI module.
//!
//! Owns the milestone list it renders plus the fields of the milestone editor.
//! One instance per window, so two windows can edit different milestones of the
//! same project without the shell swapping state around each call.

use lilia_feature_roadmap::{MilestoneUpdatePatch, ProjectRoadmap, RoadmapServiceKey};
use lilia_kernel::FeatureId;

use crate::application::ProjectWorkspaceSurface;
use crate::runtime_shell::ShellProjectPage;
pub mod view;
use crate::ui_module::{ShellEffect, UiModule, UiModuleContext, UiModuleOutcome};
use view::{RoadmapCard, RoadmapTask, RoadmapViewSnapshot};

/// The roadmap domain's own message vocabulary.
#[derive(Debug, Clone)]
pub enum RoadmapMessage {
    Open,
    Refresh,
    Select(String),
    TitleChanged(String),
    DescriptionChanged(String),
    DueDateChanged(String),
    Create,
    Save,
    CycleStatus,
    Move(isize),
    Delete,
    ToggleTask(String),
}

#[derive(Default)]
pub struct RoadmapModule {
    roadmap: ProjectRoadmap,
    selected: Option<String>,
    title: String,
    description: String,
    due_date: String,
    loaded_title: String,
    loaded_description: String,
    loaded_due_date: String,
    error: Option<String>,
}

impl RoadmapModule {
    pub fn feature_id() -> FeatureId {
        FeatureId::new("lilia.roadmap").expect("the roadmap feature id is not blank")
    }

    pub fn roadmap(&self) -> &ProjectRoadmap {
        &self.roadmap
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Restores the milestone a window remembered, falling back to the first one
    /// when the saved id no longer exists.
    pub fn restore_selection(&mut self, milestone_id: Option<String>) {
        self.selected = milestone_id
            .filter(|selected| self.has(selected))
            .or_else(|| {
                self.roadmap
                    .milestones
                    .first()
                    .map(|milestone| milestone.id.clone())
            });
        self.load_selected();
    }

    /// The milestone the editor fields belong to, if it still exists.
    fn milestone(&self) -> Option<&lilia_feature_roadmap::Milestone> {
        let selected = self.selected.as_deref()?;
        self.roadmap
            .milestones
            .iter()
            .find(|milestone| milestone.id == selected)
    }

    fn has(&self, milestone_id: &str) -> bool {
        self.roadmap
            .milestones
            .iter()
            .any(|milestone| milestone.id == milestone_id)
    }

    fn refresh(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let Some(project_id) = cx.selected_project() else {
            self.roadmap = ProjectRoadmap::default();
            self.selected = None;
            self.load_selected();
            return UiModuleOutcome::dirty();
        };
        let service = match cx
            .kernel()
            .service::<RoadmapServiceKey>()
            .map_err(|error| format!("路线图服务不可用：{error}"))
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match service.list(&project_id) {
            Ok(roadmap) => {
                self.roadmap = roadmap;
                self.apply_loaded_records();
                self.error = None;
            }
            Err(error) => self.error = Some(format!("无法读取路线图：{error}")),
        }
        UiModuleOutcome::dirty()
    }

    fn select(&mut self, milestone_id: String) -> UiModuleOutcome {
        if !self.has(&milestone_id) {
            return UiModuleOutcome::clean();
        }
        self.selected = Some(milestone_id);
        self.load_selected();
        UiModuleOutcome::dirty()
    }

    fn editor_is_dirty(&self) -> bool {
        if self.selected.is_none() {
            return !self.title.is_empty()
                || !self.description.is_empty()
                || !self.due_date.is_empty();
        }
        self.title != self.loaded_title
            || self.description != self.loaded_description
            || self.due_date != self.loaded_due_date
    }

    fn apply_loaded_records(&mut self) {
        let selected_missing = self
            .selected
            .as_deref()
            .is_some_and(|selected| !self.has(selected));
        if selected_missing {
            if self.editor_is_dirty() {
                self.selected = None;
                return;
            }
            self.selected = self
                .roadmap
                .milestones
                .first()
                .map(|milestone| milestone.id.clone());
            self.load_selected();
            return;
        }
        if self.editor_is_dirty() {
            return;
        }
        if self
            .selected
            .as_deref()
            .is_none_or(|selected| !self.has(selected))
        {
            self.selected = self
                .roadmap
                .milestones
                .first()
                .map(|milestone| milestone.id.clone());
        }
        self.load_selected();
    }

    /// Refills the editor fields from the selected milestone, so an edit always
    /// starts from what is stored rather than from the previous selection.
    fn load_selected(&mut self) {
        match self.milestone() {
            Some(milestone) => {
                let title = milestone.title.clone();
                let description = milestone.description.clone();
                let due_date = milestone
                    .due_date
                    .map(crate::desktop::format_civil_date)
                    .unwrap_or_default();
                self.title = title.clone();
                self.description = description.clone();
                self.due_date = due_date.clone();
                self.loaded_title = title;
                self.loaded_description = description;
                self.loaded_due_date = due_date;
            }
            None => {
                self.title.clear();
                self.description.clear();
                self.due_date.clear();
                self.loaded_title.clear();
                self.loaded_description.clear();
                self.loaded_due_date.clear();
            }
        }
    }

    fn create(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let Some(project_id) = cx.selected_project() else {
            return UiModuleOutcome::clean();
        };
        let service = match cx
            .kernel()
            .service::<RoadmapServiceKey>()
            .map_err(|error| format!("路线图服务不可用：{error}"))
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        if self.editor_is_dirty() && self.selected.is_none() {
            return self.save(cx);
        }
        match service.create(&project_id, "新里程碑") {
            Ok(milestone) => {
                self.refresh(cx);
                self.select(milestone.id)
            }
            Err(error) => {
                self.error = Some(format!("无法创建里程碑：{error}"));
                UiModuleOutcome::dirty()
            }
        }
    }

    fn save(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let Some(project_id) = cx.selected_project() else {
            return UiModuleOutcome::clean();
        };
        if self.title.trim().is_empty() {
            return UiModuleOutcome::clean();
        }
        let due_date = match crate::desktop::parse_civil_date_update(&self.due_date) {
            Ok(due_date) => due_date,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        let service = match cx
            .kernel()
            .service::<RoadmapServiceKey>()
            .map_err(|error| format!("路线图服务不可用：{error}"))
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        let patch = MilestoneUpdatePatch {
            title: Some(self.title.clone()),
            description: Some(self.description.clone()),
            status: None,
            due_date,
        };
        let result = match self.selected.clone() {
            Some(milestone_id) if self.has(&milestone_id) => {
                service.update(&project_id, &milestone_id, patch)
            }
            _ => match service.create(&project_id, &self.title) {
                Ok(milestone) => {
                    self.selected = Some(milestone.id.clone());
                    self.refresh(cx);
                    match service.update(&project_id, &milestone.id, patch) {
                        Ok(updated) => Ok(updated),
                        Err(error) => {
                            self.refresh(cx);
                            Err(error)
                        }
                    }
                }
                Err(error) => Err(error),
            },
        };
        match result {
            Ok(_) => {
                let outcome = self.refresh(cx);
                self.load_selected();
                outcome
            }
            Err(error) => {
                self.error = Some(format!("无法保存里程碑：{error}"));
                UiModuleOutcome::dirty()
            }
        }
    }

    fn cycle_status(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let (Some(project_id), Some(milestone_id)) = (cx.selected_project(), self.selected.clone())
        else {
            return UiModuleOutcome::clean();
        };
        let Some(status) = self
            .milestone()
            .map(|milestone| crate::desktop::next_milestone_status(milestone.status))
        else {
            return UiModuleOutcome::clean();
        };
        let service = match cx
            .kernel()
            .service::<RoadmapServiceKey>()
            .map_err(|error| format!("路线图服务不可用：{error}"))
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        let patch = MilestoneUpdatePatch {
            status: Some(status),
            ..MilestoneUpdatePatch::default()
        };
        match service.update(&project_id, &milestone_id, patch) {
            Ok(_) => self.refresh(cx),
            Err(error) => {
                self.error = Some(format!("无法更新里程碑状态：{error}"));
                UiModuleOutcome::dirty()
            }
        }
    }

    fn move_selected(&mut self, offset: isize, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let (Some(project_id), Some(milestone_id)) = (cx.selected_project(), self.selected.clone())
        else {
            return UiModuleOutcome::clean();
        };
        let Some(index) = self
            .roadmap
            .milestones
            .iter()
            .position(|milestone| milestone.id == milestone_id)
        else {
            return UiModuleOutcome::clean();
        };
        let target = index as isize + offset;
        if target < 0 || target >= self.roadmap.milestones.len() as isize {
            return UiModuleOutcome::clean();
        }
        let service = match cx
            .kernel()
            .service::<RoadmapServiceKey>()
            .map_err(|error| format!("路线图服务不可用：{error}"))
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        let mut ids = self
            .roadmap
            .milestones
            .iter()
            .map(|milestone| milestone.id.clone())
            .collect::<Vec<_>>();
        ids.swap(index, target as usize);
        match service.reorder(&project_id, ids) {
            Ok(_) => self.refresh(cx),
            Err(error) => {
                self.error = Some(format!("无法调整里程碑顺序：{error}"));
                UiModuleOutcome::dirty()
            }
        }
    }

    fn delete(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let (Some(project_id), Some(milestone_id)) = (cx.selected_project(), self.selected.clone())
        else {
            return UiModuleOutcome::clean();
        };
        let service = match cx
            .kernel()
            .service::<RoadmapServiceKey>()
            .map_err(|error| format!("路线图服务不可用：{error}"))
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match service.delete(&project_id, &milestone_id) {
            Ok(_) => {
                self.selected = None;
                self.title.clear();
                self.description.clear();
                self.due_date.clear();
                self.loaded_title.clear();
                self.loaded_description.clear();
                self.loaded_due_date.clear();
                self.refresh(cx)
            }
            Err(error) => {
                self.error = Some(format!("无法删除里程碑：{error}"));
                UiModuleOutcome::dirty()
            }
        }
    }

    fn toggle_task(&mut self, task_id: String, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let (Some(project_id), Some(milestone_id)) = (cx.selected_project(), self.selected.clone())
        else {
            return UiModuleOutcome::clean();
        };
        let service = match cx
            .kernel()
            .service::<RoadmapServiceKey>()
            .map_err(|error| format!("路线图服务不可用：{error}"))
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        let mut task_ids = self
            .roadmap
            .links
            .iter()
            .filter(|link| link.milestone_id == milestone_id)
            .map(|link| link.task_id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        if !task_ids.remove(&task_id) {
            task_ids.insert(task_id);
        }
        match service.set_tasks(&project_id, &milestone_id, task_ids.into_iter().collect()) {
            Ok(_) => self.refresh(cx),
            Err(error) => {
                self.error = Some(format!("无法更新关联任务：{error}"));
                UiModuleOutcome::dirty()
            }
        }
    }
}

impl UiModule for RoadmapModule {
    type Projection<'a> = crate::ui_module::projection::RoadmapProjection<'a>;

    type Message = RoadmapMessage;

    fn feature(&self) -> FeatureId {
        Self::feature_id()
    }

    fn reduce(&mut self, message: Self::Message, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        match message {
            RoadmapMessage::Open => UiModuleOutcome::effect(ShellEffect::RevealProjectSurface(
                ProjectWorkspaceSurface::Roadmap,
            )),
            RoadmapMessage::Refresh => self.refresh(cx),
            RoadmapMessage::Select(milestone_id) => self.select(milestone_id),
            RoadmapMessage::TitleChanged(value) => {
                self.title = value;
                self.error = None;
                UiModuleOutcome::dirty()
            }
            RoadmapMessage::DescriptionChanged(value) => {
                self.description = value;
                self.error = None;
                UiModuleOutcome::dirty()
            }
            RoadmapMessage::DueDateChanged(value) => {
                self.due_date = value;
                self.error = None;
                UiModuleOutcome::dirty()
            }
            RoadmapMessage::Create => self.create(cx),
            RoadmapMessage::Save => self.save(cx),
            RoadmapMessage::CycleStatus => self.cycle_status(cx),
            RoadmapMessage::Move(offset) => self.move_selected(offset, cx),
            RoadmapMessage::Delete => self.delete(cx),
            RoadmapMessage::ToggleTask(task_id) => self.toggle_task(task_id, cx),
        }
    }

    fn invalidate(
        &mut self,
        envelope: &lilia_kernel::EventEnvelope,
        cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        let Some(event) = envelope.downcast::<lilia_feature_roadmap::RoadmapChanged>() else {
            return UiModuleOutcome::clean();
        };
        if cx.selected_project().as_ref() != Some(&event.project_id) {
            return UiModuleOutcome::clean();
        }
        self.refresh(cx)
    }

    fn project_fields(&self, cx: &UiModuleContext<'_>, into: Self::Projection<'_>) {
        if !cx.shows(ShellProjectPage::Roadmap) {
            return;
        }
        *into.roadmap = RoadmapViewSnapshot {
            project_id: cx.selected_project().map(|id| id.as_str().to_owned()),
            selected: self.selected.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            due_date: self.due_date.clone(),
            status_label: self
                .milestone()
                .map(|milestone| {
                    crate::desktop::milestone_status_label(milestone.status).to_owned()
                })
                .unwrap_or_default(),
            error: self.error.clone(),
            cards: self
                .roadmap
                .milestones
                .iter()
                .map(|milestone| RoadmapCard {
                    id: milestone.id.clone(),
                    title: milestone.title.clone(),
                    subtitle: format!(
                        "{} · {}",
                        crate::desktop::milestone_status_label(milestone.status),
                        milestone
                            .due_date
                            .map(|due| due.to_string())
                            .unwrap_or_else(|| "无截止日期".to_owned())
                    ),
                })
                .collect(),
            tasks: {
                let selected = self.selected.clone();
                let linked = self
                    .roadmap
                    .links
                    .iter()
                    .filter(|link| selected.as_deref() == Some(link.milestone_id.as_str()))
                    .map(|link| link.task_id.clone())
                    .collect::<std::collections::BTreeSet<_>>();
                cx.workspace_snapshot()
                    .map(|snapshot| {
                        snapshot
                            .tasks
                            .into_iter()
                            .map(|task| {
                                let id = task.id.as_str().to_owned();
                                RoadmapTask {
                                    linked: linked.contains(&id),
                                    id,
                                    title: if task.title.trim().is_empty() {
                                        "未命名会话".to_owned()
                                    } else {
                                        task.title
                                    },
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            },
        };
    }
}
