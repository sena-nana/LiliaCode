use lilia_kernel::{
    EventBus, Feature, FeatureContext, FeatureId, JobContext, KernelError, ServiceKey, ServiceRef,
};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lilia_contracts::{ProjectId, TaskId};

use crate::application::{DesktopApplication, DesktopApplicationError};
use crate::application::{
    WorktreeChanged, WorktreeOperationCompleted, WorktreeOperationFailed, WorktreePreferencesPort,
};

pub use lilia_feature_worktree::*;

pub trait WorktreeTaskPort: Send + Sync {
    fn task(&self, id: &TaskId) -> Result<lilia_contracts::ProductTask, DesktopApplicationError>;
    fn project(&self, id: &ProjectId) -> Result<lilia_contracts::Project, DesktopApplicationError>;
    fn archive(&self, id: &TaskId) -> Result<(), DesktopApplicationError>;
}
impl WorktreeTaskPort for lilia_feature_task::ProjectTaskService {
    fn task(&self, id: &TaskId) -> Result<lilia_contracts::ProductTask, DesktopApplicationError> {
        Ok(self.get_task(id)?)
    }
    fn project(&self, id: &ProjectId) -> Result<lilia_contracts::Project, DesktopApplicationError> {
        Ok(self.get_project(id)?)
    }
    fn archive(&self, id: &TaskId) -> Result<(), DesktopApplicationError> {
        self.set_task_archived(id, true)?;
        Ok(())
    }
}
pub trait GitWorktreePort: Send + Sync {
    fn run(&self, repository: &Path, args: &[OsString]) -> Result<String, DesktopWorktreeError>;
}
pub struct NativeGitWorktreePort;
impl GitWorktreePort for NativeGitWorktreePort {
    fn run(&self, repository: &Path, args: &[OsString]) -> Result<String, DesktopWorktreeError> {
        run_git(repository, args)
    }
}
pub trait WorktreeRuntimePort: Send + Sync {
    fn ensure_turn_idle(&self, task_id: &TaskId) -> Result<(), DesktopApplicationError>;
}
type WorktreeReservations = Arc<(
    Mutex<BTreeMap<String, std::thread::ThreadId>>,
    std::sync::Condvar,
)>;
struct WorktreeReservation {
    reservations: WorktreeReservations,
    key: String,
}

#[cfg(test)]
pub(crate) struct WorktreeOccupation {
    _reservation: WorktreeReservation,
}
impl Drop for WorktreeReservation {
    fn drop(&mut self) {
        let (state, wake) = &*self.reservations;
        state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.key);
        wake.notify_all();
    }
}
#[derive(Clone)]
pub struct DesktopWorktreeService {
    store: Arc<DesktopWorktreeStore>,
    tasks: Arc<dyn WorktreeTaskPort>,
    preferences: Arc<dyn WorktreePreferencesPort>,
    git: Arc<dyn GitWorktreePort>,
    runtime: Arc<dyn WorktreeRuntimePort>,
    events: EventBus,
    reservations: WorktreeReservations,
    operation_context: Option<(JobContext, Arc<std::sync::atomic::AtomicBool>)>,
    operation_scope: Option<(Option<ProjectId>, Option<PathBuf>)>,
    operation_archived: Arc<std::sync::atomic::AtomicBool>,
}
fn worktree_input(message: &str) -> DesktopApplicationError {
    DesktopApplicationError::InvalidInput {
        field: "worktree",
        message: message.into(),
    }
}
impl DesktopWorktreeService {
    pub fn new(
        store: Arc<DesktopWorktreeStore>,
        tasks: Arc<dyn WorktreeTaskPort>,
        preferences: Arc<dyn WorktreePreferencesPort>,
        git: Arc<dyn GitWorktreePort>,
        runtime: Arc<dyn WorktreeRuntimePort>,
        events: EventBus,
    ) -> Self {
        Self {
            store,
            tasks,
            preferences,
            git,
            runtime,
            events,
            reservations: Arc::new((Mutex::new(BTreeMap::new()), std::sync::Condvar::new())),
            operation_context: None,
            operation_scope: None,
            operation_archived: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    pub fn store(&self) -> Arc<DesktopWorktreeStore> {
        self.store.clone()
    }

    pub(crate) fn ensure_task_idle(&self, task_id: &TaskId) -> Result<(), DesktopApplicationError> {
        self.runtime.ensure_turn_idle(task_id)?;
        self.ensure_reservation_idle(task_id)
    }

    #[cfg(test)]
    pub(crate) fn occupy_task_for_test(
        &self,
        task_id: &TaskId,
    ) -> Result<WorktreeOccupation, DesktopApplicationError> {
        Ok(WorktreeOccupation {
            _reservation: self.reserve(format!("task:{}", task_id.as_str()), None)?,
        })
    }

    fn ensure_reservation_idle(&self, task_id: &TaskId) -> Result<(), DesktopApplicationError> {
        let state = self
            .reservations
            .0
            .lock()
            .map_err(|_| DesktopApplicationError::StateUnavailable("worktree operations"))?;
        if state.contains_key(&format!("task:{}", task_id.as_str())) {
            return Err(DesktopApplicationError::InvalidInput {
                field: "worktree",
                message: "请等待当前工作树操作结束。".into(),
            });
        }
        Ok(())
    }
    fn get_task(
        &self,
        id: &TaskId,
    ) -> Result<lilia_contracts::ProductTask, DesktopApplicationError> {
        self.tasks.task(id)
    }
    fn get_project(
        &self,
        id: &ProjectId,
    ) -> Result<lilia_contracts::Project, DesktopApplicationError> {
        self.tasks.project(id)
    }
    fn reserve(
        &self,
        key: String,
        context: Option<&JobContext>,
    ) -> Result<WorktreeReservation, DesktopApplicationError> {
        let (state, wake) = &*self.reservations;
        let mut state = state
            .lock()
            .map_err(|_| DesktopApplicationError::StateUnavailable("worktree operations"))?;
        let owner = std::thread::current().id();
        loop {
            if context.is_some_and(JobContext::is_cancelled) {
                return Err(worktree_input(
                    "worktree operation cancelled before changes",
                ));
            }
            match state.get(&key) {
                None => {
                    state.insert(key.clone(), owner);
                    return Ok(WorktreeReservation {
                        reservations: self.reservations.clone(),
                        key,
                    });
                }
                Some(current) if *current == owner => {
                    return Err(worktree_input("worktree operation already in progress"));
                }
                _ => {
                    state = wake
                        .wait_timeout(state, std::time::Duration::from_millis(25))
                        .map_err(|_| {
                            DesktopApplicationError::StateUnavailable("worktree operations")
                        })?
                        .0
                }
            }
        }
    }
    fn mutate<T>(
        &self,
        id: &TaskId,
        repository: bool,
        context: Option<&JobContext>,
        operation: impl FnOnce(&Self) -> Result<T, DesktopApplicationError>,
    ) -> Result<T, DesktopApplicationError> {
        self.runtime.ensure_turn_idle(id)?;
        {
            let state =
                self.reservations.0.lock().map_err(|_| {
                    DesktopApplicationError::StateUnavailable("worktree operations")
                })?;
            if state
                .values()
                .any(|owner| *owner == std::thread::current().id())
            {
                return Err(worktree_input(
                    "worktree operation cannot reenter during an active operation",
                ));
            }
        }
        let task_guard = self.reserve(format!("task:{}", id.as_str()), context)?;
        let task = self.get_task(id)?;
        if task.archived {
            return Err(worktree_input("task is archived"));
        }
        let mut scope_base = None;
        let repo_guard = if repository {
            let (_, project, base) = self.task_repository(id)?;
            let common = run_git_text(
                &base,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            )?;
            let common = canonical_path(Path::new(common.trim()), "git common directory")?;
            let guard = self.reserve(format!("repo:{}", normalized_path(&common)), context)?;
            let (_, current_project, current_base) = self.task_repository(id)?;
            if project != current_project
                || canonical_path(&base, "repository")?
                    != canonical_path(&current_base, "repository")?
            {
                return Err(worktree_input("task repository changed while waiting"));
            }
            scope_base = Some(canonical_path(&base, "repository")?);
            Some(guard)
        } else {
            None
        };
        if context.is_some_and(JobContext::is_cancelled) {
            return Err(worktree_input(
                "worktree operation cancelled before changes",
            ));
        }
        let before = self.store.for_task(id)?;
        let intent_before = self.store.initial_intent(id)?;
        let merge_before = self.store.merge_attempted(id)?;
        let mut scoped = self.clone();
        scoped.operation_scope = Some((task.project_id.clone(), scope_base));
        scoped.operation_archived = Arc::new(std::sync::atomic::AtomicBool::new(false));
        scoped.operation_context = context.map(|context| {
            (
                context.clone(),
                Arc::new(std::sync::atomic::AtomicBool::new(false)),
            )
        });
        let result = operation(&scoped);
        let result = match scoped.validate_operation_scope(id) {
            Ok(()) => result,
            Err(error) => Err(error),
        };
        let changed = self
            .store
            .for_task(id)
            .map(|after| after != before)
            .unwrap_or(true)
            || self
                .store
                .initial_intent(id)
                .map(|after| after != intent_before)
                .unwrap_or(true);
        let changed = changed
            || self
                .store
                .merge_attempted(id)
                .map(|value| value != merge_before)
                .unwrap_or(true);
        drop(repo_guard);
        drop(task_guard);
        if changed {
            self.events.publish(WorktreeChanged {
                task_id: id.clone(),
            });
        }
        result
    }
    fn validate_operation_scope(&self, id: &TaskId) -> Result<(), DesktopApplicationError> {
        let Some((project_id, base)) = &self.operation_scope else {
            return Ok(());
        };
        let task = self.get_task(id)?;
        if task.project_id != *project_id
            || (task.archived
                && !self
                    .operation_archived
                    .load(std::sync::atomic::Ordering::Acquire))
        {
            return Err(worktree_input(
                "task changed during worktree operation; completed Git changes were retained",
            ));
        }
        if let Some(project_id) = project_id {
            let project = self.get_project(project_id)?;
            if project.archive != lilia_contracts::ProjectArchiveState::Active {
                return Err(worktree_input(
                    "project was archived during worktree operation",
                ));
            }
            if let Some(base) = base {
                let current = project.workspace_path.or_else(|| {
                    project
                        .git_workspace
                        .and_then(|workspace| workspace.worktree_path)
                });
                let Some(current) = current else {
                    return Err(worktree_input(
                        "project repository changed during worktree operation",
                    ));
                };
                if canonical_path(Path::new(&current), "current repository")? != *base {
                    return Err(worktree_input(
                        "project repository changed during worktree operation",
                    ));
                }
            }
        }
        Ok(())
    }
    fn begin_changes(&self) -> Result<(), DesktopApplicationError> {
        if let Some((context, started)) = &self.operation_context {
            if !started.load(std::sync::atomic::Ordering::Acquire) {
                if context.is_cancelled() {
                    return Err(worktree_input(
                        "worktree operation cancelled before changes",
                    ));
                }
                started.store(true, std::sync::atomic::Ordering::Release);
            }
        }
        Ok(())
    }
    fn git_run(&self, base: &Path, args: &[OsString]) -> Result<String, DesktopApplicationError> {
        self.begin_changes()?;
        Ok(self.git.run(base, args)?)
    }
    fn git_text(&self, base: &Path, args: &[&str]) -> Result<String, DesktopApplicationError> {
        self.git_run(base, &args.iter().map(OsString::from).collect::<Vec<_>>())
    }
    fn remove_worktree(
        &self,
        id: &TaskId,
        binding: &DesktopTaskWorktree,
        base: &Path,
        path: &Path,
        status: DesktopWorktreeStatus,
    ) -> Result<(), DesktopApplicationError> {
        // The directory removal commits before branch cleanup. Persist that fact even if branch deletion fails.
        self.git_run(
            base,
            &[
                OsString::from("worktree"),
                OsString::from("remove"),
                path.as_os_str().to_owned(),
            ],
        )?;
        self.store.mark_status(id, status)?;
        self.git_text(base, &["branch", "-d", &binding.branch_name])?;
        Ok(())
    }
    fn archive_result(
        &self,
        id: &TaskId,
        mut result: DesktopWorktreeMergeResult,
    ) -> Result<DesktopWorktreeMergeResult, DesktopApplicationError> {
        self.validate_operation_scope(id)?;
        self.tasks.archive(id)?;
        self.operation_archived
            .store(true, std::sync::atomic::Ordering::Release);
        result.archived = true;
        Ok(result)
    }
    fn archive_locked(
        &self,
        id: &TaskId,
        merge: bool,
    ) -> Result<DesktopWorktreeMergeResult, DesktopApplicationError> {
        if let Some(binding) = self.store.for_task(id)? {
            if binding.status != DesktopWorktreeStatus::Active {
                let (_, project, base) = self.task_repository(id)?;
                if project != binding.project_id
                    || canonical_path(&base, "repository")?
                        != canonical_path(Path::new(&binding.base_repo_path), "bound repository")?
                {
                    return Err(worktree_input(
                        "task repository no longer owns this worktree",
                    ));
                }
                if Path::new(&binding.worktree_path).exists() {
                    return Err(worktree_input("removed worktree path is still present"));
                }
                if git_status_success(
                    &base,
                    &[
                        OsString::from("show-ref"),
                        OsString::from("--verify"),
                        OsString::from("--quiet"),
                        OsString::from(format!("refs/heads/{}", binding.branch_name)),
                    ],
                )? {
                    self.git_text(&base, &["branch", "-d", &binding.branch_name])?;
                }
                return self.archive_result(
                    id,
                    DesktopWorktreeMergeResult {
                        merged: binding.status == DesktopWorktreeStatus::Merged,
                        removed: true,
                        archived: false,
                        message: "Archived task after completed worktree removal".into(),
                    },
                );
            }
        }
        let result = if merge {
            self.merge_task_worktree_and_archive_locked(id)?
        } else {
            self.cleanup_task_worktree_and_archive_locked(id)?
        };
        self.archive_result(id, result)
    }
}

impl DesktopWorktreeService {
    fn set_initial_worktree_intent_locked(
        &self,
        task_id: &TaskId,
        selection: Option<&DesktopInitialWorktreeSelection>,
    ) -> Result<(), DesktopApplicationError> {
        self.begin_changes()?;
        let worktrees = &self.store;
        if let Some(selection) = selection {
            worktrees.save_initial_intent(task_id, selection)?;
        } else {
            worktrees.clear_initial_intent(task_id)?;
        }
        Ok(())
    }

    pub fn initial_worktree_intent(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<DesktopInitialWorktreeSelection>, DesktopApplicationError> {
        self.get_task(task_id)?;
        Ok(self.store.initial_intent(task_id)?)
    }

    fn retry_initial_worktree_locked(
        &self,
        task_id: &TaskId,
    ) -> Result<bool, DesktopApplicationError> {
        let Some(selection) = self.initial_worktree_intent(task_id)? else {
            return Ok(false);
        };
        if self.task_worktree(task_id)?.is_none() {
            match selection {
                DesktopInitialWorktreeSelection::Create => {
                    self.create_task_worktree_locked(task_id, None)?;
                }
                DesktopInitialWorktreeSelection::Existing(path) => {
                    self.attach_task_worktree_locked(task_id, &path)?;
                }
            }
        }
        self.set_initial_worktree_intent_locked(task_id, None)?;
        Ok(true)
    }

    pub(crate) fn ensure_initial_worktree_ready(
        &self,
        task_id: &TaskId,
    ) -> Result<(), DesktopApplicationError> {
        if self.initial_worktree_intent(task_id)?.is_some() {
            return Err(DesktopWorktreeError::InitialPreparationPending(task_id.clone()).into());
        }
        Ok(())
    }

    pub fn task_workspace_path(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<String>, DesktopApplicationError> {
        if let Some(worktree) = self.task_worktree(task_id)? {
            return Ok(Some(worktree.worktree_path));
        }
        let task = self.get_task(task_id)?;
        let Some(project_id) = task.project_id else {
            return Ok(None);
        };
        let project = self.get_project(&project_id)?;
        Ok(project.workspace_path.or_else(|| {
            project
                .git_workspace
                .and_then(|workspace| workspace.worktree_path)
        }))
    }

    pub fn task_worktree(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<DesktopTaskWorktree>, DesktopApplicationError> {
        let task = self.get_task(task_id)?;
        let binding = self.store.active_for_task(task_id)?;
        if let Some(binding) = &binding {
            if binding.project_id != task.project_id {
                return Err(worktree_input("task project no longer owns this worktree"));
            }
            let (_, _, base) = self.task_repository(task_id)?;
            if canonical_path(&base, "base repository")?
                != canonical_path(Path::new(&binding.base_repo_path), "bound repository")?
            {
                return Err(worktree_input(
                    "project repository no longer owns this worktree",
                ));
            }
        }
        Ok(binding)
    }

    pub fn list_task_repository_worktrees(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<DesktopWorktreeListItem>, DesktopApplicationError> {
        let (_, _, base) = self.task_repository(task_id)?;
        let base = canonical_path(&base, "base repository")?;
        let base_text = normalized_path(&base);
        let bound_paths = self.store.active_bound_paths()?;
        Ok(list_git_worktrees(&base)?
            .into_iter()
            .map(|item| {
                let item_path = canonical_path(Path::new(&item.path), "worktree")
                    .map(|path| normalized_path(&path))
                    .unwrap_or(item.path);
                DesktopWorktreeListItem {
                    is_main: item_path == base_text,
                    is_task_bound: bound_paths.contains(&item_path),
                    path: item_path,
                    head: item.head,
                    branch: item.branch,
                    bare: item.bare,
                    detached: item.detached,
                    prunable: item.prunable,
                    locked: item.locked,
                }
            })
            .collect())
    }

    fn create_task_worktree_locked(
        &self,
        task_id: &TaskId,
        parent_directory: Option<&Path>,
    ) -> Result<DesktopTaskWorktree, DesktopApplicationError> {
        let (task, project_id, base) = self.task_repository(task_id)?;
        if self.task_worktree(task_id)?.is_some() {
            return Err(DesktopWorktreeError::AlreadyBound(task_id.clone()).into());
        }
        ensure_git_repo(&base)?;
        let base = canonical_path(&base, "base repository")?;
        let base_branch = current_branch(&base)?;
        let preferred_parent = self
            .preferences
            .worktree_settings()?
            .parent_dir
            .map(PathBuf::from);
        let parent = parent_directory
            .map(Path::to_path_buf)
            .or(preferred_parent)
            .unwrap_or_else(|| {
                base.parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| base.clone())
            });
        let parent = canonical_path(&parent, "worktree parent directory")?;
        let slug = task_title_slug(&task.title, task_id);
        let target = unique_worktree_target(&parent, &slug);
        let branch = unique_branch_name(&base, &slug)?;
        self.git_run(
            &base,
            &[
                OsString::from("worktree"),
                OsString::from("add"),
                OsString::from("-b"),
                OsString::from(&branch),
                target.as_os_str().to_owned(),
                OsString::from(&base_branch),
            ],
        )?;
        let worktree = match canonical_path(&target, "created worktree") {
            Ok(path) => path,
            Err(error) => {
                rollback_created_worktree(&base, &target, &branch);
                return Err(error.into());
            }
        };
        let base_text = normalized_path(&base);
        let worktree_text = normalized_path(&worktree);
        self.begin_changes()?;
        let saved = self.store.save_active(
            task_id,
            project_id.as_ref(),
            &base_text,
            &worktree_text,
            &branch,
            &base_branch,
        );
        let saved = match saved {
            Ok(saved) => saved,
            Err(error) => {
                rollback_created_worktree(&base, &worktree, &branch);
                return Err(error.into());
            }
        };
        Ok(saved)
    }

    fn attach_task_worktree_locked(
        &self,
        task_id: &TaskId,
        worktree_path: &Path,
    ) -> Result<DesktopTaskWorktree, DesktopApplicationError> {
        let (_, project_id, base) = self.task_repository(task_id)?;
        if self.task_worktree(task_id)?.is_some() {
            return Err(DesktopWorktreeError::AlreadyBound(task_id.clone()).into());
        }
        ensure_git_repo(&base)?;
        ensure_git_repo(worktree_path)?;
        let base = canonical_path(&base, "base repository")?;
        let worktree = canonical_path(worktree_path, "worktree")?;
        if base == worktree {
            return Err(DesktopWorktreeError::MainRepositoryCannotBeAttached.into());
        }
        let worktree_text = normalized_path(&worktree);
        let registered = list_git_worktrees(&base)?
            .into_iter()
            .find(|item| {
                canonical_path(Path::new(&item.path), "worktree").is_ok_and(|path| path == worktree)
            })
            .ok_or_else(|| DesktopWorktreeError::NotRegistered(worktree_text.clone()))?;
        let branch = registered
            .branch
            .filter(|branch| !branch.trim().is_empty())
            .ok_or_else(|| DesktopWorktreeError::Detached(worktree_text.clone()))?;
        let base_branch = current_branch(&base)?;
        self.begin_changes()?;
        let saved = self.store.save_active(
            task_id,
            project_id.as_ref(),
            &normalized_path(&base),
            &worktree_text,
            &branch,
            &base_branch,
        )?;
        Ok(saved)
    }

    fn clear_task_worktree_locked(
        &self,
        task_id: &TaskId,
    ) -> Result<bool, DesktopApplicationError> {
        self.task_worktree(task_id)?;
        self.begin_changes()?;
        let changed = self
            .store
            .mark_status(task_id, DesktopWorktreeStatus::Removed)?;
        if changed {}
        Ok(changed)
    }

    fn cleanup_task_worktree_and_archive_locked(
        &self,
        task_id: &TaskId,
    ) -> Result<DesktopWorktreeMergeResult, DesktopApplicationError> {
        let worktree = self
            .task_worktree(task_id)?
            .ok_or_else(|| DesktopWorktreeError::NotBound(task_id.clone()))?;
        let base = PathBuf::from(&worktree.base_repo_path);
        let worktree_path = PathBuf::from(&worktree.worktree_path);
        ensure_git_repo(&base)?;
        ensure_git_repo(&worktree_path)?;
        ensure_clean(&base, "base repository")?;
        ensure_clean(&worktree_path, "worktree")?;
        if branch_unique_commit_count(&worktree_path, &worktree.base_branch)? > 0 {
            return Err(DesktopWorktreeError::UnmergedCommits.into());
        }
        self.remove_worktree(
            task_id,
            &worktree,
            &base,
            &worktree_path,
            DesktopWorktreeStatus::Removed,
        )?;
        self.finish_worktree_archive(
            task_id,
            DesktopWorktreeStatus::Removed,
            false,
            "Removed the worktree without unique commits and archived the task",
        )
    }

    fn merge_task_worktree_and_archive_locked(
        &self,
        task_id: &TaskId,
    ) -> Result<DesktopWorktreeMergeResult, DesktopApplicationError> {
        let worktree = self
            .task_worktree(task_id)?
            .ok_or_else(|| DesktopWorktreeError::NotBound(task_id.clone()))?;
        let base = PathBuf::from(&worktree.base_repo_path);
        let worktree_path = PathBuf::from(&worktree.worktree_path);
        ensure_git_repo(&base)?;
        ensure_git_repo(&worktree_path)?;
        ensure_clean(&base, "base repository")?;
        ensure_clean(&worktree_path, "worktree")?;
        let has_unique = branch_unique_commit_count(&worktree_path, &worktree.base_branch)? > 0;
        if !has_unique && !self.store.merge_attempted(task_id)? {
            return Err(DesktopWorktreeError::NoUniqueCommits.into());
        }
        if has_unique {
            self.begin_changes()?;
            self.store.mark_merge_attempted(task_id)?;
            if current_branch(&base)? != worktree.base_branch {
                self.git_text(&base, &["checkout", &worktree.base_branch])?;
            }
            self.git_text(&base, &["merge", "--no-ff", &worktree.branch_name])?;
        }
        self.remove_worktree(
            task_id,
            &worktree,
            &base,
            &worktree_path,
            DesktopWorktreeStatus::Merged,
        )?;
        self.finish_worktree_archive(
            task_id,
            DesktopWorktreeStatus::Merged,
            true,
            "Merged the worktree branch, removed the worktree, and archived the task",
        )
    }

    fn task_repository(
        &self,
        task_id: &TaskId,
    ) -> Result<(lilia_contracts::ProductTask, Option<ProjectId>, PathBuf), DesktopApplicationError>
    {
        let task = self.get_task(task_id)?;
        if task.archived {
            return Err(worktree_input("task is archived"));
        }
        let project_id = task
            .project_id
            .clone()
            .ok_or_else(|| DesktopWorktreeError::TaskHasNoProject(task_id.clone()))?;
        let project = self.get_project(&project_id)?;
        if project.archive != lilia_contracts::ProjectArchiveState::Active {
            return Err(worktree_input("project is archived"));
        }
        let workspace = project
            .workspace_path
            .as_deref()
            .or_else(|| {
                project
                    .git_workspace
                    .as_ref()
                    .and_then(|workspace| workspace.worktree_path.as_deref())
            })
            .filter(|path| !path.trim().is_empty())
            .ok_or_else(|| DesktopWorktreeError::ProjectHasNoWorkspace(project_id.clone()))?;
        Ok((task, Some(project_id), PathBuf::from(workspace)))
    }

    fn finish_worktree_archive(
        &self,
        task_id: &TaskId,
        status: DesktopWorktreeStatus,
        merged: bool,
        message: &str,
    ) -> Result<DesktopWorktreeMergeResult, DesktopApplicationError> {
        self.store.mark_status(task_id, status)?;
        Ok(DesktopWorktreeMergeResult {
            merged,
            removed: true,
            archived: false,
            message: message.to_owned(),
        })
    }
}

fn list_git_worktrees(base_repo_path: &Path) -> Result<Vec<GitWorktree>, DesktopWorktreeError> {
    ensure_git_repo(base_repo_path)?;
    let output = run_git_text(base_repo_path, &["worktree", "list", "--porcelain"])?;
    Ok(parse_worktree_porcelain(&output))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use lilia_contracts::{ProductEntity, ProductTask, Project};
    use lilia_service::ServiceAuthority;
    use lilia_storage::Db;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::application::{
        DesktopApplicationConfig, DesktopHost, DesktopHostAction, DesktopHostContext,
        DesktopHostError, DesktopHostResult, DesktopTurnRequest,
    };

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

    fn initialize_repository(root: &Path) {
        fs::create_dir_all(root).unwrap();
        run_git_text(root, &["init", "-b", "main"]).unwrap();
        run_git_text(root, &["config", "user.email", "native@example.invalid"]).unwrap();
        run_git_text(root, &["config", "user.name", "Native Test"]).unwrap();
        run_git_text(root, &["config", "core.autocrlf", "false"]).unwrap();
        fs::write(root.join("README.md"), "native\n").unwrap();
        run_git_text(root, &["add", "README.md"]).unwrap();
        run_git_text(root, &["commit", "-m", "initial"]).unwrap();
    }

    fn application(repo: &Path) -> (DesktopApplication, TaskId) {
        let instance_id = Uuid::new_v4();
        let authority = ServiceAuthority::bootstrap_in_memory_named(
            format!("test:desktop-worktree:{instance_id}"),
            format!("desktop-worktree-test:{instance_id}"),
        )
        .unwrap();
        let project_id = ProjectId::new("worktree-project").unwrap();
        let mut project = Project::new(project_id.clone(), "Worktree project").unwrap();
        project.workspace_path = Some(normalized_path(repo));
        authority
            .client()
            .unwrap()
            .products()
            .create_entity(ProductEntity::Project(project))
            .unwrap();
        let task_id = TaskId::new("worktree-task").unwrap();
        authority
            .client()
            .unwrap()
            .products()
            .create_entity(ProductEntity::Task(
                ProductTask::new(task_id.clone(), Some(project_id), "Native worktree").unwrap(),
            ))
            .unwrap();
        let home = repo
            .parent()
            .expect("the test repository lives under a temp parent")
            .join(format!("home-{instance_id}"));
        fs::create_dir_all(&home).unwrap();
        let application = DesktopApplication::from_authority(
            DesktopApplicationConfig::new(&home, "liliacode.test").unwrap(),
            authority,
            Arc::new(NoopHost),
        )
        .unwrap();
        (application, task_id)
    }

    struct PausedGit {
        entered: std::sync::mpsc::Sender<()>,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
        first: std::sync::atomic::AtomicBool,
    }
    impl GitWorktreePort for PausedGit {
        fn run(&self, repo: &Path, args: &[OsString]) -> Result<String, DesktopWorktreeError> {
            let result = run_git(repo, args)?;
            self.entered.send(()).unwrap();
            if self.first.swap(false, std::sync::atomic::Ordering::SeqCst) {
                self.release
                    .lock()
                    .unwrap()
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
            }
            Ok(result)
        }
    }
    fn paused_git(
        service: &mut DesktopWorktreeService,
    ) -> (std::sync::mpsc::Receiver<()>, std::sync::mpsc::Sender<()>) {
        let (entered, observed) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        service.git = Arc::new(PausedGit {
            entered,
            release: Mutex::new(wait),
            first: true.into(),
        });
        (observed, release)
    }
    fn add_task_project(app: &DesktopApplication, repo: &Path, name: &str) -> (ProjectId, TaskId) {
        let project_id = ProjectId::new(format!("project-{name}")).unwrap();
        let task_id = TaskId::new(format!("task-{name}")).unwrap();
        let mut project = Project::new(project_id.clone(), name).unwrap();
        project.workspace_path = Some(normalized_path(repo));
        let client = app.inner.authority.client().unwrap();
        client
            .products()
            .create_entity(ProductEntity::Project(project))
            .unwrap();
        client
            .products()
            .create_entity(ProductEntity::Task(
                ProductTask::new(task_id.clone(), Some(project_id.clone()), name).unwrap(),
            ))
            .unwrap();
        (project_id, task_id)
    }
    #[test]
    fn different_tasks_using_main_and_linked_bases_share_the_common_git_reservation() {
        let root = TempDir::new().unwrap();
        let repo = root.path().join("repo");
        initialize_repository(&repo);
        let linked = root.path().join("linked");
        run_git(
            &repo,
            &[
                "worktree".into(),
                "add".into(),
                "-b".into(),
                "linked-base".into(),
                linked.as_os_str().to_owned(),
            ],
        )
        .unwrap();
        let (app, first_id) = application(&repo);
        let (_, second_id) = add_task_project(&app, &linked, "second");
        let mut service = app.worktree_service();
        let (entered, release) = paused_git(&mut service);
        let first = {
            let service = service.clone();
            let id = first_id.clone();
            std::thread::spawn(move || service.create_task_worktree(&id, None))
        };
        entered
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let second = {
            let service = service.clone();
            let id = second_id.clone();
            std::thread::spawn(move || service.create_task_worktree(&id, None))
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !service
            .reservations
            .0
            .lock()
            .unwrap()
            .contains_key(&format!("task:{}", second_id.as_str()))
        {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(
            entered
                .recv_timeout(std::time::Duration::from_millis(150))
                .is_err()
        );
        release.send(()).unwrap();
        assert!(first.join().unwrap().is_ok());
        assert!(second.join().unwrap().is_ok());
        entered
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(list_git_worktrees(&repo).unwrap().len(), 4);
        assert_ne!(
            service
                .task_worktree(&first_id)
                .unwrap()
                .unwrap()
                .worktree_path,
            service
                .task_worktree(&second_id)
                .unwrap()
                .unwrap()
                .worktree_path
        );
    }
    #[test]
    fn task_move_during_git_retains_original_binding_but_rejects_old_scope_completion() {
        let root = TempDir::new().unwrap();
        let repo = root.path().join("repo");
        let other = root.path().join("other");
        initialize_repository(&repo);
        initialize_repository(&other);
        let (app, id) = application(&repo);
        let (other_project, _) = add_task_project(&app, &other, "other");
        let original_project = app.get_task(&id).unwrap().project_id;
        let mut service = app.worktree_service();
        let (entered, release) = paused_git(&mut service);
        let worker = {
            let service = service.clone();
            let id = id.clone();
            std::thread::spawn(move || service.create_task_worktree(&id, None))
        };
        entered
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        app.inner
            .project_tasks
            .move_task(
                &id,
                lilia_feature_task::DesktopTaskMove {
                    target_project_id: Some(other_project),
                    target_parent_id: None,
                },
            )
            .unwrap();
        release.send(()).unwrap();
        assert!(worker.join().unwrap().is_err());
        let binding = service.store.for_task(&id).unwrap().unwrap();
        assert_eq!(binding.project_id, original_project);
        assert!(Path::new(&binding.worktree_path).is_dir());
        assert!(service.task_worktree(&id).is_err());
        assert!(service.cleanup_task_worktree_and_archive(&id).is_err());
        assert_eq!(list_git_worktrees(&other).unwrap().len(), 1);
        assert!(!app.get_task(&id).unwrap().archived);
    }
    struct FailRemovalOnce(std::sync::atomic::AtomicBool);
    impl GitWorktreePort for FailRemovalOnce {
        fn run(&self, repo: &Path, args: &[OsString]) -> Result<String, DesktopWorktreeError> {
            if args.first().is_some_and(|arg| arg == "worktree")
                && args.get(1).is_some_and(|arg| arg == "remove")
                && self.0.swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                return Err(DesktopWorktreeError::Git {
                    command: "worktree remove".into(),
                    message: "injected removal failure".into(),
                });
            }
            run_git(repo, args)
        }
    }
    #[test]
    fn successful_merge_followed_by_failed_removal_resumes_without_merging_twice() {
        let root = TempDir::new().unwrap();
        let repo = root.path().join("repo");
        initialize_repository(&repo);
        let (app, id) = application(&repo);
        let mut service = app.worktree_service();
        let binding = service.create_task_worktree(&id, None).unwrap();
        let path = Path::new(&binding.worktree_path);
        fs::write(path.join("change.txt"), "committed").unwrap();
        run_git_text(path, &["add", "change.txt"]).unwrap();
        run_git_text(path, &["commit", "-m", "change"]).unwrap();
        service.git = Arc::new(FailRemovalOnce(true.into()));
        assert!(service.merge_task_worktree_and_archive(&id).is_err());
        assert!(repo.join("change.txt").exists());
        assert!(path.exists());
        assert!(service.store.merge_attempted(&id).unwrap());
        let merged_head = run_git_text(&repo, &["rev-parse", "HEAD"]).unwrap();
        assert!(service.merge_task_worktree_and_archive(&id).unwrap().merged);
        assert_eq!(
            run_git_text(&repo, &["rev-parse", "HEAD"]).unwrap(),
            merged_head
        );
        assert!(!path.exists());
        assert!(app.get_task(&id).unwrap().archived);
    }
    #[test]
    fn legacy_worktree_table_upgrade_preserves_binding_and_merge_retry_progress_survives_reopen() {
        let root = TempDir::new().unwrap();
        let path = root.path().join("old.db");
        let db = Db::open(&path).unwrap();
        db.lock().execute_batch("CREATE TABLE task_worktrees(task_id TEXT PRIMARY KEY, project_id TEXT, base_repo_path TEXT NOT NULL, worktree_path TEXT NOT NULL UNIQUE, branch_name TEXT NOT NULL, base_branch TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL); INSERT INTO task_worktrees VALUES('legacy','project','repo','tree','branch','main','active',42,43);").unwrap();
        let store = DesktopWorktreeStore::from_db(db).unwrap();
        let id = TaskId::new("legacy").unwrap();
        let original = store.for_task(&id).unwrap().unwrap();
        assert_eq!(original.created_at, 42);
        assert_eq!(original.updated_at, 43);
        assert!(!store.merge_attempted(&id).unwrap());
        store.mark_merge_attempted(&id).unwrap();
        drop(store);
        let reopened = DesktopWorktreeStore::from_db(Db::open(&path).unwrap()).unwrap();
        assert_eq!(reopened.for_task(&id).unwrap(), Some(original));
        assert!(reopened.merge_attempted(&id).unwrap());
    }
    struct ControlledTasks {
        delegate: Arc<dyn WorktreeTaskPort>,
        fail_archive: std::sync::atomic::AtomicBool,
        wrong_project: std::sync::atomic::AtomicBool,
    }
    impl WorktreeTaskPort for ControlledTasks {
        fn task(&self, id: &TaskId) -> Result<ProductTask, DesktopApplicationError> {
            let mut task = self.delegate.task(id)?;
            if self.wrong_project.load(std::sync::atomic::Ordering::SeqCst) {
                task.project_id = Some(ProjectId::new("different-project").unwrap());
            }
            Ok(task)
        }
        fn project(&self, id: &ProjectId) -> Result<Project, DesktopApplicationError> {
            self.delegate.project(id)
        }
        fn archive(&self, id: &TaskId) -> Result<(), DesktopApplicationError> {
            if self.fail_archive.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(worktree_input("archive unavailable"));
            }
            self.delegate.archive(id)
        }
    }
    #[test]
    fn concurrent_creates_share_the_registered_store_and_publish_after_unlock() {
        let root = TempDir::new().unwrap();
        let repo = root.path().join("repo");
        initialize_repository(&repo);
        let (app, id) = application(&repo);
        let service = app.worktree_service();
        let kernel =
            lilia_kernel::Kernel::with_events(lilia_kernel::Journal::new(), service.events.clone());
        kernel
            .mount(Arc::new(WorktreeFeature::new(
                service.store(),
                Arc::new(service.clone()),
            )))
            .unwrap();
        kernel
            .mount(Arc::new(WorktreeServiceFeature::new(service.clone())))
            .unwrap();
        assert!(Arc::ptr_eq(
            &kernel.service::<WorktreeStoreKey>().unwrap(),
            &service.store
        ));
        let mounted = kernel.service::<WorktreeServiceKey>().unwrap();
        assert!(Arc::ptr_eq(&mounted.reservations, &service.reservations));
        let observed = Arc::new(Mutex::new(Vec::new()));
        let sink = observed.clone();
        let reader = service.clone();
        let _subscription = service.events.on::<WorktreeChanged, _>(None, move |event| {
            let binding = reader.task_worktree(&event.task_id).unwrap().unwrap();
            // A same-task mutation during notification must be outside the reservation.
            reader
                .set_initial_worktree_intent(&event.task_id, None)
                .unwrap();
            sink.lock().unwrap().push(binding);
        });
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let service = mounted.clone();
            let id = id.clone();
            let barrier = barrier.clone();
            let parent = root.path().to_owned();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                service.create_task_worktree(&id, Some(&parent))
            }));
        }
        barrier.wait();
        let results: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(observed.lock().unwrap().len(), 1);
        assert_eq!(list_git_worktrees(&repo).unwrap().len(), 2);
        assert_eq!(
            service.task_worktree(&id).unwrap().unwrap(),
            observed.lock().unwrap()[0]
        );
    }
    #[test]
    fn wrong_repository_and_reassigned_task_cannot_replace_or_remove_binding() {
        let root = TempDir::new().unwrap();
        let repo = root.path().join("repo");
        let other = root.path().join("other");
        initialize_repository(&repo);
        initialize_repository(&other);
        let (app, id) = application(&repo);
        let mut service = app.worktree_service();
        let foreign = root.path().join("foreign");
        run_git(
            &other,
            &[
                "worktree".into(),
                "add".into(),
                "-b".into(),
                "foreign".into(),
                foreign.as_os_str().to_owned(),
            ],
        )
        .unwrap();
        assert!(service.attach_task_worktree(&id, &foreign).is_err());
        assert!(service.store.for_task(&id).unwrap().is_none());
        let binding = service
            .create_task_worktree(&id, Some(root.path()))
            .unwrap();
        let tasks = Arc::new(ControlledTasks {
            delegate: service.tasks.clone(),
            fail_archive: false.into(),
            wrong_project: true.into(),
        });
        service.tasks = tasks;
        assert!(service.clear_task_worktree(&id).is_err());
        assert!(service.cleanup_task_worktree_and_archive(&id).is_err());
        assert_eq!(service.store.for_task(&id).unwrap(), Some(binding.clone()));
        assert!(Path::new(&binding.worktree_path).exists());
        assert!(foreign.exists());
    }
    #[test]
    fn archive_failure_preserves_committed_removal_and_retry_only_archives() {
        let root = TempDir::new().unwrap();
        let repo = root.path().join("repo");
        initialize_repository(&repo);
        let (app, id) = application(&repo);
        let mut service = app.worktree_service();
        let tasks = Arc::new(ControlledTasks {
            delegate: service.tasks.clone(),
            fail_archive: true.into(),
            wrong_project: false.into(),
        });
        service.tasks = tasks.clone();
        let binding = service
            .create_task_worktree(&id, Some(root.path()))
            .unwrap();
        let reader = service.store();
        let (tx, rx) = std::sync::mpsc::channel();
        let _subscription = service.events.on::<WorktreeChanged, _>(None, move |event| {
            tx.send(reader.for_task(&event.task_id).unwrap().unwrap().status)
                .unwrap();
        });
        assert!(service.cleanup_task_worktree_and_archive(&id).is_err());
        assert!(!Path::new(&binding.worktree_path).exists());
        assert_eq!(rx.try_recv().unwrap(), DesktopWorktreeStatus::Removed);
        assert!(rx.try_recv().is_err());
        assert!(!app.get_task(&id).unwrap().archived);
        tasks
            .fail_archive
            .store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(
            service
                .cleanup_task_worktree_and_archive(&id)
                .unwrap()
                .archived
        );
        assert!(app.get_task(&id).unwrap().archived);
        assert_eq!(list_git_worktrees(&repo).unwrap().len(), 1);
    }
    struct CancelAfterGit {
        context: JobContext,
    }
    impl GitWorktreePort for CancelAfterGit {
        fn run(&self, repo: &Path, args: &[OsString]) -> Result<String, DesktopWorktreeError> {
            let result = run_git(repo, args)?;
            self.context.request_cancel();
            Ok(result)
        }
    }
    #[test]
    fn cancellation_before_git_has_no_effect_but_after_git_keeps_committed_truth() {
        let root = TempDir::new().unwrap();
        let repo = root.path().join("repo");
        initialize_repository(&repo);
        let (app, id) = application(&repo);
        let mut service = app.worktree_service();
        let context = JobContext::new();
        context.request_cancel();
        let request = WorktreeRequest {
            task_id: id.as_str().into(),
            operation: WorktreeOperationRequest::Create,
        };
        assert!(
            service
                .operate_with_context(request.clone(), &context)
                .is_err()
        );
        assert_eq!(list_git_worktrees(&repo).unwrap().len(), 1);
        assert!(service.store.for_task(&id).unwrap().is_none());
        let context = JobContext::new();
        service.git = Arc::new(CancelAfterGit {
            context: context.clone(),
        });
        service.operate_with_context(request, &context).unwrap();
        assert!(context.is_cancelled());
        let binding = service.task_worktree(&id).unwrap().unwrap();
        assert!(Path::new(&binding.worktree_path).exists());
        assert_eq!(list_git_worktrees(&repo).unwrap().len(), 2);
    }
    #[test]
    fn parses_porcelain_worktree_state() {
        let worktrees = parse_worktree_porcelain(
            "worktree D:/repo\nHEAD abc\nbranch refs/heads/main\n\nworktree D:/repo-wt\nHEAD def\nbranch refs/heads/lilia/task\nlocked reason\n",
        );

        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].branch.as_deref(), Some("main"));
        assert_eq!(worktrees[1].path, "D:/repo-wt");
        assert_eq!(worktrees[1].branch.as_deref(), Some("lilia/task"));
        assert!(worktrees[1].locked);
    }

    #[test]
    fn create_merge_remove_and_archive_uses_real_git_and_product_state() {
        let root = TempDir::new().unwrap();
        let repo = root.path().join("repo");
        initialize_repository(&repo);
        let (application, task_id) = application(&repo);

        let binding = application
            .create_task_worktree(&task_id, Some(root.path()))
            .unwrap();
        assert_eq!(
            application
                .task_workspace_path(&task_id)
                .unwrap()
                .as_deref(),
            Some(binding.worktree_path.as_str())
        );
        assert!(Path::new(&binding.worktree_path).is_dir());
        assert!(binding.branch_name.starts_with("lilia/native-worktree-"));
        let listed = application
            .list_task_repository_worktrees(&task_id)
            .unwrap();
        assert_eq!(listed.iter().filter(|item| item.is_main).count(), 1);
        assert_eq!(listed.iter().filter(|item| item.is_task_bound).count(), 1);

        let worktree_path = PathBuf::from(&binding.worktree_path);
        fs::write(worktree_path.join("native.txt"), "complete\n").unwrap();
        run_git_text(&worktree_path, &["add", "native.txt"]).unwrap();
        run_git_text(&worktree_path, &["commit", "-m", "native worktree"]).unwrap();

        let result = application
            .merge_task_worktree_and_archive(&task_id)
            .unwrap();

        assert!(result.merged);
        assert!(result.removed);
        assert!(result.archived);
        assert!(!worktree_path.exists());
        assert!(repo.join("native.txt").is_file());
        assert!(application.get_task(&task_id).unwrap().archived);
        assert_eq!(application.task_worktree(&task_id).unwrap(), None);
    }

    #[test]
    fn initial_worktree_intent_survives_reopen_until_explicitly_cleared() {
        let root = TempDir::new().unwrap();
        let database = root.path().join("worktrees.db");
        let task_id = TaskId::new("pending-worktree").unwrap();
        let selection = DesktopInitialWorktreeSelection::Existing(root.path().join("existing"));
        {
            let store = DesktopWorktreeStore::from_db(Db::open(&database).unwrap()).unwrap();
            store.save_initial_intent(&task_id, &selection).unwrap();
        }

        let store = DesktopWorktreeStore::from_db(Db::open(&database).unwrap()).unwrap();
        assert_eq!(store.initial_intent(&task_id).unwrap(), Some(selection));
        assert!(store.clear_initial_intent(&task_id).unwrap());
        assert_eq!(store.initial_intent(&task_id).unwrap(), None);
    }

    #[test]
    fn reserved_draft_can_store_initial_worktree_intent_before_the_task_exists() {
        let root = TempDir::new().unwrap();
        let repo = root.path().join("repo");
        initialize_repository(&repo);
        let (application, _) = application(&repo);
        let reserved = TaskId::new("reserved-draft").unwrap();
        application
            .set_initial_worktree_intent(&reserved, Some(&DesktopInitialWorktreeSelection::Create))
            .unwrap();
        assert_eq!(
            application
                .worktree_service()
                .store()
                .initial_intent(&reserved)
                .unwrap(),
            Some(DesktopInitialWorktreeSelection::Create)
        );
        application
            .set_initial_worktree_intent(&reserved, None)
            .unwrap();
        assert_eq!(
            application
                .worktree_service()
                .store()
                .initial_intent(&reserved)
                .unwrap(),
            None
        );
    }

    #[test]
    fn pending_initial_worktree_blocks_turns_and_retry_clears_the_gate() {
        let root = TempDir::new().unwrap();
        let repo = root.path().join("repo");
        initialize_repository(&repo);
        let (application, task_id) = application(&repo);
        application
            .set_initial_worktree_intent(&task_id, Some(&DesktopInitialWorktreeSelection::Create))
            .unwrap();

        assert!(matches!(
            application.start_composer_turn(&task_id),
            Err(DesktopApplicationError::Worktree(
                DesktopWorktreeError::InitialPreparationPending(ref pending)
            )) if pending == &task_id
        ));
        assert!(application.retry_initial_worktree(&task_id).unwrap());
        assert_eq!(application.initial_worktree_intent(&task_id).unwrap(), None);
        assert!(application.task_worktree(&task_id).unwrap().is_some());
    }

    #[test]
    fn mutate_and_idle_check_reject_live_or_queued_turns() {
        let root = TempDir::new().unwrap();
        let repo = root.path().join("repo");
        initialize_repository(&repo);
        let (application, task_id) = application(&repo);
        application.inner.agent.enqueue_idempotent(
            DesktopTurnRequest::new(task_id.clone(), "live"),
            "turn-live".to_owned(),
        );

        let error = application
            .create_task_worktree(&task_id, None)
            .unwrap_err();
        assert!(matches!(
            error,
            DesktopApplicationError::InvalidInput {
                field: "worktree",
                ref message
            } if message == "请先停止当前任务，再更改工作树。"
        ));
        assert!(matches!(
            application.worktree_service().ensure_task_idle(&task_id),
            Err(DesktopApplicationError::InvalidInput {
                field: "worktree",
                ref message
            }) if message == "请先停止当前任务，再更改工作树。"
        ));
        assert!(application.ensure_task_worktree_idle(&task_id).is_ok());
    }
}

impl DesktopWorktreeService {
    pub fn set_initial_worktree_intent(
        &self,
        task_id: &TaskId,
        selection: Option<&DesktopInitialWorktreeSelection>,
    ) -> Result<(), DesktopApplicationError> {
        match self.get_task(task_id) {
            Ok(task) if task.archived => Err(worktree_input("task is archived")),
            Ok(_) => self.mutate(task_id, false, None, |service| {
                service.set_initial_worktree_intent_locked(task_id, selection)
            }),
            Err(DesktopApplicationError::Product(lilia_contracts::ProductError::NotFound {
                ..
            })) => self.set_initial_worktree_intent_locked(task_id, selection),
            Err(error) => Err(error),
        }
    }
    pub fn retry_initial_worktree(
        &self,
        task_id: &TaskId,
    ) -> Result<bool, DesktopApplicationError> {
        self.mutate(task_id, true, None, |service| {
            service.retry_initial_worktree_locked(task_id)
        })
    }
    pub fn create_task_worktree(
        &self,
        task_id: &TaskId,
        parent_directory: Option<&Path>,
    ) -> Result<DesktopTaskWorktree, DesktopApplicationError> {
        self.mutate(task_id, true, None, |service| {
            service.create_task_worktree_locked(task_id, parent_directory)
        })
    }
    pub fn attach_task_worktree(
        &self,
        task_id: &TaskId,
        worktree_path: &Path,
    ) -> Result<DesktopTaskWorktree, DesktopApplicationError> {
        self.mutate(task_id, true, None, |service| {
            service.attach_task_worktree_locked(task_id, worktree_path)
        })
    }
    pub fn clear_task_worktree(&self, task_id: &TaskId) -> Result<bool, DesktopApplicationError> {
        self.mutate(task_id, true, None, |service| {
            service.clear_task_worktree_locked(task_id)
        })
    }
    pub fn cleanup_task_worktree_and_archive(
        &self,
        task_id: &TaskId,
    ) -> Result<DesktopWorktreeMergeResult, DesktopApplicationError> {
        self.mutate(task_id, true, None, |service| {
            service.archive_locked(task_id, false)
        })
    }
    pub fn merge_task_worktree_and_archive(
        &self,
        task_id: &TaskId,
    ) -> Result<DesktopWorktreeMergeResult, DesktopApplicationError> {
        self.mutate(task_id, true, None, |service| {
            service.archive_locked(task_id, true)
        })
    }
}

impl DesktopApplication {
    pub fn worktree_service(&self) -> DesktopWorktreeService {
        self.inner.worktree_service.clone()
    }
    pub fn set_initial_worktree_intent(
        &self,
        task_id: &TaskId,
        selection: Option<&DesktopInitialWorktreeSelection>,
    ) -> Result<(), DesktopApplicationError> {
        self.inner
            .worktree_service
            .set_initial_worktree_intent(task_id, selection)
    }
    pub fn retry_initial_worktree(
        &self,
        task_id: &TaskId,
    ) -> Result<bool, DesktopApplicationError> {
        self.inner.worktree_service.retry_initial_worktree(task_id)
    }
    pub fn create_task_worktree(
        &self,
        task_id: &TaskId,
        parent_directory: Option<&Path>,
    ) -> Result<DesktopTaskWorktree, DesktopApplicationError> {
        self.inner
            .worktree_service
            .create_task_worktree(task_id, parent_directory)
    }
    pub fn attach_task_worktree(
        &self,
        task_id: &TaskId,
        worktree_path: &Path,
    ) -> Result<DesktopTaskWorktree, DesktopApplicationError> {
        self.inner
            .worktree_service
            .attach_task_worktree(task_id, worktree_path)
    }
    pub fn clear_task_worktree(&self, task_id: &TaskId) -> Result<bool, DesktopApplicationError> {
        self.inner.worktree_service.clear_task_worktree(task_id)
    }
    pub fn cleanup_task_worktree_and_archive(
        &self,
        task_id: &TaskId,
    ) -> Result<DesktopWorktreeMergeResult, DesktopApplicationError> {
        self.inner
            .worktree_service
            .cleanup_task_worktree_and_archive(task_id)
    }
    pub fn merge_task_worktree_and_archive(
        &self,
        task_id: &TaskId,
    ) -> Result<DesktopWorktreeMergeResult, DesktopApplicationError> {
        self.inner
            .worktree_service
            .merge_task_worktree_and_archive(task_id)
    }
    pub fn initial_worktree_intent(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<DesktopInitialWorktreeSelection>, DesktopApplicationError> {
        self.inner.worktree_service.initial_worktree_intent(task_id)
    }

    pub(crate) fn ensure_task_worktree_idle(
        &self,
        task_id: &TaskId,
    ) -> Result<(), DesktopApplicationError> {
        self.inner.worktree_service.ensure_reservation_idle(task_id)
    }
    pub fn ensure_initial_worktree_ready(
        &self,
        task_id: &TaskId,
    ) -> Result<(), DesktopApplicationError> {
        self.inner
            .worktree_service
            .ensure_initial_worktree_ready(task_id)
    }
    pub fn task_workspace_path(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<String>, DesktopApplicationError> {
        self.inner.worktree_service.task_workspace_path(task_id)
    }
    pub fn task_worktree(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<DesktopTaskWorktree>, DesktopApplicationError> {
        self.inner.worktree_service.task_worktree(task_id)
    }
    pub fn list_task_repository_worktrees(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<DesktopWorktreeListItem>, DesktopApplicationError> {
        self.inner
            .worktree_service
            .list_task_repository_worktrees(task_id)
    }
}

impl WorktreePort for DesktopWorktreeService {
    fn operate(&self, request: WorktreeRequest) -> Result<(), String> {
        self.operate_with_context(request, &JobContext::new())
    }
    fn operate_with_context(
        &self,
        request: WorktreeRequest,
        context: &JobContext,
    ) -> Result<(), String> {
        let id = TaskId::new(request.task_id).map_err(|error| error.to_string())?;
        let result = self.mutate(&id, true, Some(context), |service| {
            match request.operation {
                WorktreeOperationRequest::Create => {
                    service.create_task_worktree_locked(&id, None).map(|_| ())
                }
                WorktreeOperationRequest::Attach { path } => {
                    service.attach_task_worktree_locked(&id, &path).map(|_| ())
                }
                WorktreeOperationRequest::Clear => {
                    service.clear_task_worktree_locked(&id).map(|_| ())
                }
                WorktreeOperationRequest::CleanupAndArchive => {
                    service.archive_locked(&id, false).map(|_| ())
                }
                WorktreeOperationRequest::MergeAndArchive => {
                    service.archive_locked(&id, true).map(|_| ())
                }
            }
        });
        match result {
            Ok(()) => {
                self.events
                    .publish(WorktreeOperationCompleted { task_id: id });
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                self.events.publish(WorktreeOperationFailed {
                    task_id: id,
                    message: message.clone(),
                });
                Err(message)
            }
        }
    }
}
pub struct WorktreeServiceKey;
impl ServiceKey for WorktreeServiceKey {
    type Value = DesktopWorktreeService;
    const NAME: &'static str = "lilia.worktree.operations";
}
pub struct WorktreeServiceFeature {
    service: DesktopWorktreeService,
}
impl WorktreeServiceFeature {
    pub fn new(service: DesktopWorktreeService) -> Self {
        Self { service }
    }
}
impl Feature for WorktreeServiceFeature {
    fn id(&self) -> FeatureId {
        FeatureId::new("lilia.feature.worktree-operations").expect("nonempty feature id")
    }
    fn provides(&self) -> Vec<ServiceRef> {
        vec![ServiceRef::of::<WorktreeServiceKey>()]
    }
    fn mount(&self, cx: &mut FeatureContext<'_>) -> Result<(), KernelError> {
        cx.provide::<WorktreeServiceKey>(self.service.clone())
    }
}
