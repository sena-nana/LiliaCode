use std::fs;
use std::path::{Component, Path, PathBuf};

use lilia_contracts::{Project, ProjectId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectContext {
    pub project_id: ProjectId,
    pub workspace_root: PathBuf,
    pub worktree_root: Option<PathBuf>,
}

impl ProjectContext {
    pub fn from_project(project: &Project) -> Result<Self, ProjectContextError> {
        let workspace_root = project
            .workspace_path
            .as_ref()
            .ok_or_else(|| ProjectContextError::MissingWorkspace(project.id.clone()))
            .map(PathBuf::from)?;
        validate_absolute_root(&workspace_root)?;
        let worktree_root = project
            .git_workspace
            .as_ref()
            .and_then(|git| git.worktree_path.as_ref())
            .map(PathBuf::from)
            .map(|path| {
                validate_absolute_root(&path)?;
                Ok(path)
            })
            .transpose()?;
        Ok(Self {
            project_id: project.id.clone(),
            workspace_root,
            worktree_root,
        })
    }

    pub fn active_root(&self) -> &Path {
        self.worktree_root
            .as_deref()
            .unwrap_or(&self.workspace_root)
    }

    pub fn resolve_relative(&self, relative: &Path) -> Result<PathBuf, ProjectContextError> {
        if relative.as_os_str().is_empty() || relative.is_absolute() {
            return Err(ProjectContextError::InvalidRelativePath(
                relative.to_path_buf(),
            ));
        }
        for component in relative.components() {
            if !matches!(component, Component::Normal(_) | Component::CurDir) {
                return Err(ProjectContextError::InvalidRelativePath(
                    relative.to_path_buf(),
                ));
            }
        }
        let root = self.canonical_active_root()?;
        let mut candidate = self.active_root().join(relative);
        let mut missing = Vec::new();
        loop {
            match fs::canonicalize(&candidate) {
                Ok(resolved) => {
                    let mut resolved = normalize_canonical_path(resolved);
                    for component in missing.iter().rev() {
                        resolved.push(component);
                    }
                    if !resolved.starts_with(&root) {
                        return Err(ProjectContextError::PathEscapesWorkspace(
                            relative.to_path_buf(),
                        ));
                    }
                    return Ok(resolved);
                }
                Err(error) => {
                    if fs::symlink_metadata(&candidate).is_ok() {
                        return Err(ProjectContextError::Io {
                            path: candidate,
                            message: error.to_string(),
                        });
                    }
                    let Some(component) = candidate.file_name().map(|name| name.to_os_string())
                    else {
                        return Err(ProjectContextError::Io {
                            path: candidate,
                            message: error.to_string(),
                        });
                    };
                    missing.push(component);
                    if !candidate.pop() {
                        return Err(ProjectContextError::Io {
                            path: candidate,
                            message: error.to_string(),
                        });
                    }
                }
            }
        }
    }

    pub fn canonical_active_root(&self) -> Result<PathBuf, ProjectContextError> {
        fs::canonicalize(self.active_root())
            .map(normalize_canonical_path)
            .map_err(|error| ProjectContextError::Io {
                path: self.active_root().to_path_buf(),
                message: error.to_string(),
            })
    }
}

fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let value = path.to_string_lossy();
        if value.starts_with(r"\\?\") {
            return PathBuf::from(&value[4..]);
        }
    }
    path
}

fn validate_absolute_root(path: &Path) -> Result<(), ProjectContextError> {
    if !path.is_absolute() {
        return Err(ProjectContextError::WorkspaceRootMustBeAbsolute(
            path.to_path_buf(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProjectContextError {
    #[error("project `{0}` has no workspace root")]
    MissingWorkspace(ProjectId),
    #[error("workspace root must be absolute: `{0:?}`")]
    WorkspaceRootMustBeAbsolute(PathBuf),
    #[error("path must stay relative to the active project root: `{0:?}`")]
    InvalidRelativePath(PathBuf),
    #[error("path escapes the active project root: `{0:?}`")]
    PathEscapesWorkspace(PathBuf),
    #[error("failed to resolve `{path:?}`: {message}")]
    Io { path: PathBuf, message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_context_rejects_paths_that_escape_the_active_root() {
        let project = Project::new(ProjectId::new("project").unwrap(), "Project").unwrap();
        let context = ProjectContext {
            project_id: project.id,
            workspace_root: std::env::current_dir().unwrap(),
            worktree_root: None,
        };

        assert!(matches!(
            context.resolve_relative(Path::new("../secret.txt")),
            Err(ProjectContextError::InvalidRelativePath(_))
        ));
        assert_eq!(
            context.resolve_relative(Path::new("src/main.rs")).unwrap(),
            context.workspace_root.join("src/main.rs")
        );
    }

    #[cfg(unix)]
    #[test]
    fn project_context_jails_symlinks_but_allows_in_tree_targets() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        std::os::unix::fs::symlink(
            root.path().join("src/main.rs"),
            root.path().join("alias.rs"),
        )
        .unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.txt"),
            root.path().join("leak.rs"),
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
        let context = context_at(root.path().to_path_buf());

        assert_eq!(
            context.resolve_relative(Path::new("alias.rs")).unwrap(),
            fs::canonicalize(root.path().join("src/main.rs")).unwrap()
        );
        assert!(matches!(
            context.resolve_relative(Path::new("leak.rs")),
            Err(ProjectContextError::PathEscapesWorkspace(_))
        ));
        assert!(matches!(
            context.resolve_relative(Path::new("escape/secret.txt")),
            Err(ProjectContextError::PathEscapesWorkspace(_))
        ));
    }
}
