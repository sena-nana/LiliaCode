use lilia_agent::{BrowserArtifactSink, BrowserCancellation, BrowserError};
use lilia_contracts::{
    is_opaque_browser_resource_ref, AgentSessionRef, ArtifactId, ArtifactMaterializationStatus,
    ArtifactProjection, BrowserScope, ProductEntity, ProductEntityKind, ProjectionEventId, TaskId,
    TimelineProjectionCommand, TimelineProjectionEvent,
};
use lilia_service::ServiceAuthority;
use lilia_storage::{
    evaluate_artifact, ArtifactRetentionPolicy, LiliaDataPaths, TimelineProjectionRepository,
    ARTIFACT_STATUS_AVAILABLE, ARTIFACT_STATUS_EXPIRED, ARTIFACT_STATUS_INACCESSIBLE,
    ARTIFACT_STATUS_PINNED,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(1);

/// Product-backed persistence for browser screenshots.
pub struct DesktopBrowserArtifactSink {
    authority: ServiceAuthority,
    home: PathBuf,
    projection_sequences: Mutex<std::collections::BTreeMap<String, u64>>,
    /// Product lookup, materialization and command submission must be one
    /// critical section so concurrent captures cannot both create a recovery
    /// artifact for the same task/hash.
    commit_lock: Mutex<()>,
}

impl DesktopBrowserArtifactSink {
    pub fn new(authority: ServiceAuthority, home: impl Into<PathBuf>) -> Self {
        Self {
            authority,
            home: home.into(),
            projection_sequences: Mutex::new(std::collections::BTreeMap::new()),
            commit_lock: Mutex::new(()),
        }
    }

    fn next_projection_sequence(&self, session: &AgentSessionRef) -> u64 {
        let persisted = self
            .authority
            .projection_cursor_for_session(session)
            .unwrap_or_default();
        let mut sequences = self
            .projection_sequences
            .lock()
            .expect("browser projection sequence lock");
        let next = sequences
            .get(session.as_str())
            .copied()
            .unwrap_or(persisted)
            .saturating_add(1);
        sequences.insert(session.as_str().to_owned(), next);
        next
    }

    fn project_digest(project_id: &str) -> String {
        digest_hex(project_id.as_bytes())
    }

    fn task_digest(task_id: &str) -> String {
        digest_hex(task_id.as_bytes())
    }

    fn materialized_matches(path: &Path, expected_hash: &str) -> bool {
        path.is_file()
            && fs::read(path)
                .map(|bytes| digest_hex(&bytes) == expected_hash)
                .unwrap_or(false)
    }

    fn is_sha256_hex(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    fn artifact_root(&self, scope: &BrowserScope) -> PathBuf {
        LiliaDataPaths::from_home(&self.home)
            .product_artifacts_root()
            .join(Self::project_digest(scope.project_id.as_str()))
            .join(Self::task_digest(scope.task_id.as_str()))
            .join("screenshots")
    }

    fn resource_ref_matches_scope(value: &str, scope: &BrowserScope, content_hash: &str) -> bool {
        let Some(path) = value.strip_prefix("resource://browser/") else {
            return false;
        };
        let mut parts = path.split('/');
        let Some(project_digest) = parts.next() else {
            return false;
        };
        let Some(task_digest) = parts.next() else {
            return false;
        };
        let Some(file_name) = parts.next() else {
            return false;
        };
        is_opaque_browser_resource_ref(value)
            && Self::is_sha256_hex(project_digest)
            && task_digest == Self::task_digest(scope.task_id.as_str())
            && file_name == format!("{content_hash}.png")
            && parts.next().is_none()
    }

    fn artifact_path(
        &self,
        scope: &BrowserScope,
        content_hash: &str,
        resource_ref: Option<&str>,
    ) -> PathBuf {
        let project_digest = resource_ref
            .and_then(|value| value.strip_prefix("resource://browser/"))
            .and_then(|path| {
                let mut parts = path.split('/');
                let project = parts.next()?;
                let task = parts.next()?;
                let file = parts.next()?;
                (parts.next().is_none()
                    && Self::is_sha256_hex(project)
                    && task == Self::task_digest(scope.task_id.as_str())
                    && file == format!("{content_hash}.png"))
                .then_some(project.to_owned())
            })
            .unwrap_or_else(|| Self::project_digest(scope.project_id.as_str()));
        LiliaDataPaths::from_home(&self.home)
            .product_artifacts_root()
            .join(project_digest)
            .join(Self::task_digest(scope.task_id.as_str()))
            .join("screenshots")
            .join(format!("{content_hash}.png"))
    }

    fn project(
        &self,
        scope: &BrowserScope,
        session: &str,
        turn: &str,
        command_id: &str,
        artifact: &lilia_contracts::ProductArtifact,
        sequence: u64,
        status: &str,
    ) -> Result<(), BrowserError> {
        let agent_session = AgentSessionRef::new(session.to_owned())
            .map_err(|error| BrowserError::Host(error.to_string()))?;
        let artifact_id = artifact.id.as_str().to_owned();
        let content_ref = artifact
            .resource_ref
            .clone()
            .map(|value| serde_json::json!({ "resource": value, "kind": "browser_screenshot" }));
        let projection = ArtifactProjection {
            id: format!("browser:{artifact_id}"),
            task_id: scope.task_id.clone(),
            agent_session: agent_session.clone(),
            sequence,
            turn_id: Some(turn.to_owned()),
            artifact_id: artifact_id.clone(),
            media_type: artifact.media_type.clone(),
            summary: "Browser screenshot".into(),
            kind: Some("browser_screenshot".into()),
            size_bytes: artifact.size_bytes,
            content_hash: artifact.content_hash.clone(),
            content_ref: content_ref.clone(),
            provenance: Some("browser.screenshot".into()),
            status: status.into(),
        };
        self.authority
            .apply_projection(TimelineProjectionCommand::UpsertArtifact {
                artifact: projection,
            })
            .map_err(|error| BrowserError::Host(error.to_string()))?;
        self.authority
            .apply_projection(TimelineProjectionCommand::UpsertTimelineEvent {
                event: TimelineProjectionEvent {
                    id: ProjectionEventId::new(format!("browser:{artifact_id}")),
                    task_id: scope.task_id.clone(),
                    agent_session,
                    sequence,
                    turn_id: Some(turn.to_owned()),
                    kind: "artifact".into(),
                    status: if status == "available" {
                        "success"
                    } else {
                        status
                    }
                    .into(),
                    title: "Browser screenshot".into(),
                    summary: Some("Browser screenshot captured".into()),
                    payload: serde_json::json!({
                        "artifactId": artifact_id,
                        "mediaType": artifact.media_type,
                        "sizeBytes": artifact.size_bytes,
                        "contentHash": artifact.content_hash,
                        "contentRef": content_ref,
                        "sourceEventId": command_id,
                    }),
                    projected: true,
                },
            })
            .map_err(|error| BrowserError::Host(error.to_string()))?;
        Ok(())
    }

    /// Rebuild browser rows from durable ProductArtifact/ProductEvent facts.
    /// This is safe to call repeatedly after a projection-store failure.
    pub fn rebuild_browser_projections(&self, task_id: &TaskId) -> Result<usize, BrowserError> {
        let client = self
            .authority
            .client()
            .map_err(|error| BrowserError::Host(error.to_string()))?;
        let task = client
            .products()
            .get_task(task_id)
            .map_err(|error| BrowserError::Host(error.to_string()))?;
        let Some(project_id) = task.project_id else {
            return Ok(0);
        };
        let mut repaired = 0;
        for entity in client
            .products()
            .list_entities(ProductEntityKind::Artifact)
            .map_err(|error| BrowserError::Host(error.to_string()))?
        {
            let ProductEntity::Artifact(artifact) = entity else {
                continue;
            };
            if artifact.task_id != *task_id
                || artifact.provenance.as_deref() != Some("browser.screenshot")
            {
                continue;
            }
            let Some(hash) = artifact.content_hash.as_deref() else {
                continue;
            };
            let path = if Self::is_sha256_hex(hash) {
                Some(self.artifact_path(
                    &BrowserScope {
                        project_id: project_id.clone(),
                        task_id: task_id.clone(),
                        tab_id: "rebuild".into(),
                    },
                    hash,
                    artifact.resource_ref.as_deref(),
                ))
            } else {
                None
            };
            let session = AgentSessionRef::new(artifact.agent_session.as_str().to_owned())
                .map_err(|error| BrowserError::Host(error.to_string()))?;
            let projection_id = format!("browser:{}", artifact.id.as_str());
            let runtime = self.authority.shared_runtime();
            let projections = runtime.inner().projections();
            let existing_artifact_projection = projections
                .list_artifacts_for_task(task_id)
                .into_iter()
                .find(|projection| projection.id == projection_id);
            let existing_timeline_event = projections
                .list_for_task(task_id)
                .into_iter()
                .find(|event| event.id.as_str() == projection_id);
            let status = if path
                .as_deref()
                .is_some_and(|path| Self::materialized_matches(path, hash))
            {
                if existing_artifact_projection
                    .as_ref()
                    .is_some_and(|projection| projection.status == ARTIFACT_STATUS_PINNED)
                {
                    ARTIFACT_STATUS_PINNED
                } else {
                    ARTIFACT_STATUS_AVAILABLE
                }
            } else {
                ARTIFACT_STATUS_INACCESSIBLE
            };
            let sequence = existing_artifact_projection
                .as_ref()
                .map(|projection| projection.sequence)
                .or_else(|| existing_timeline_event.as_ref().map(|event| event.sequence))
                .unwrap_or_else(|| self.next_projection_sequence(&session));
            drop(runtime);
            self.project(
                &BrowserScope {
                    project_id: project_id.clone(),
                    task_id: task_id.clone(),
                    tab_id: "rebuild".into(),
                },
                session.as_str(),
                "rebuild",
                artifact.source_event_id.as_deref().unwrap_or_default(),
                &artifact,
                sequence,
                status,
            )?;
            repaired += 1;
        }
        Ok(repaired)
    }

    /// Apply retention and remove only this task's unpinned materializations.
    /// Missing files remain represented as inaccessible projection metadata.
    pub fn cleanup_browser_artifacts_for_task(
        &self,
        task_id: &TaskId,
        now_ms: u64,
        policy: &ArtifactRetentionPolicy,
    ) -> Result<(), BrowserError> {
        let task = self
            .authority
            .client()
            .map_err(|error| BrowserError::Host(error.to_string()))?
            .products()
            .get_task(task_id)
            .map_err(|error| BrowserError::Host(error.to_string()))?;
        let Some(project_id) = task.project_id else {
            return Ok(());
        };
        // ProductArtifact is the durable source of truth. Rebuild first so a
        // projection-store failure cannot strand materialized files forever.
        self.rebuild_browser_projections(task_id)?;
        let runtime = self.authority.shared_runtime();
        let projections = runtime.inner().projections();
        let rows = projections.list_artifacts_for_task(task_id);
        let mut protected_hashes = BTreeSet::new();
        for candidate in &rows {
            if candidate.provenance.as_deref() != Some("browser.screenshot") {
                continue;
            }
            let Some(hash) = candidate.content_hash.as_deref() else {
                continue;
            };
            if candidate.status == ARTIFACT_STATUS_PINNED {
                protected_hashes.insert(hash.to_owned());
                continue;
            }
            if candidate.status != ARTIFACT_STATUS_AVAILABLE {
                continue;
            }
            let candidate_path = self.artifact_path(
                &BrowserScope {
                    project_id: project_id.clone(),
                    task_id: task_id.clone(),
                    tab_id: "cleanup".into(),
                },
                hash,
                candidate
                    .content_ref
                    .as_ref()
                    .and_then(|value| value.get("resource"))
                    .and_then(serde_json::Value::as_str),
            );
            let candidate_anchor = fs::metadata(candidate_path)
                .ok()
                .and_then(|value| value.modified().ok())
                .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|value| value.as_millis() as u64);
            if evaluate_artifact(candidate, now_ms, candidate_anchor, policy).next_status
                != ARTIFACT_STATUS_EXPIRED
            {
                protected_hashes.insert(hash.to_owned());
            }
        }
        for mut row in rows {
            if row.provenance.as_deref() != Some("browser.screenshot") {
                continue;
            }
            let Some(hash) = row.content_hash.clone() else {
                continue;
            };
            if !Self::is_sha256_hex(&hash) {
                let projection_id = row.id.clone();
                row.status = ARTIFACT_STATUS_INACCESSIBLE.into();
                projections
                    .apply(TimelineProjectionCommand::UpsertArtifact { artifact: row })
                    .map_err(|error| BrowserError::Host(error.to_string()))?;
                self.update_timeline_status_by_id(
                    projections,
                    task_id,
                    &projection_id,
                    ARTIFACT_STATUS_INACCESSIBLE,
                )?;
                continue;
            }
            let path = self.artifact_path(
                &BrowserScope {
                    project_id: project_id.clone(),
                    task_id: task_id.clone(),
                    tab_id: "cleanup".into(),
                },
                &hash,
                row.content_ref
                    .as_ref()
                    .and_then(|value| value.get("resource"))
                    .and_then(serde_json::Value::as_str),
            );
            let metadata = fs::metadata(&path).ok();
            let anchor_ms = metadata
                .as_ref()
                .and_then(|value| value.modified().ok())
                .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|value| value.as_millis() as u64);
            if metadata.is_none() || !Self::materialized_matches(&path, &hash) {
                let projection_id = row.id.clone();
                row.status = ARTIFACT_STATUS_INACCESSIBLE.into();
                projections
                    .apply(TimelineProjectionCommand::UpsertArtifact { artifact: row })
                    .map_err(|error| BrowserError::Host(error.to_string()))?;
                self.update_timeline_status_by_id(
                    projections,
                    task_id,
                    &projection_id,
                    ARTIFACT_STATUS_INACCESSIBLE,
                )?;
                continue;
            }
            if row.status == ARTIFACT_STATUS_PINNED {
                continue;
            }
            let decision = evaluate_artifact(&row, now_ms, anchor_ms, policy);
            if decision.next_status == ARTIFACT_STATUS_EXPIRED {
                let projection_id = row.id.clone();
                if !protected_hashes.contains(&hash) {
                    let _ = fs::remove_file(&path);
                }
                row.status = ARTIFACT_STATUS_EXPIRED.into();
                projections
                    .apply(TimelineProjectionCommand::UpsertArtifact { artifact: row })
                    .map_err(|error| BrowserError::Host(error.to_string()))?;
                self.update_timeline_status_by_id(
                    projections,
                    task_id,
                    &projection_id,
                    ARTIFACT_STATUS_EXPIRED,
                )?;
            } else if row.status != decision.next_status {
                row.status = decision.next_status;
                projections
                    .apply(TimelineProjectionCommand::UpsertArtifact { artifact: row })
                    .map_err(|error| BrowserError::Host(error.to_string()))?;
            }
        }
        Ok(())
    }

    fn update_timeline_status_by_id(
        &self,
        projections: &dyn TimelineProjectionRepository,
        task_id: &TaskId,
        projection_id: &str,
        status: &str,
    ) -> Result<(), BrowserError> {
        let Some(mut event) = projections
            .list_for_task(task_id)
            .into_iter()
            .find(|event| event.id.as_str() == projection_id)
        else {
            return Ok(());
        };
        event.status = if status == ARTIFACT_STATUS_AVAILABLE {
            "success".into()
        } else {
            status.into()
        };
        projections
            .apply(TimelineProjectionCommand::UpsertTimelineEvent { event })
            .map_err(|error| BrowserError::Host(error.to_string()))?;
        Ok(())
    }

    fn mark_existing_inaccessible(
        &self,
        task_id: &TaskId,
        artifact_id: &ArtifactId,
    ) -> Result<(), BrowserError> {
        let runtime = self.authority.shared_runtime();
        let projections = runtime.inner().projections();
        let projection_id = format!("browser:{}", artifact_id.as_str());
        let mut found = false;
        for mut row in projections.list_artifacts_for_task(task_id) {
            if row.id != projection_id {
                continue;
            }
            found = true;
            row.status = ARTIFACT_STATUS_INACCESSIBLE.into();
            projections
                .apply(TimelineProjectionCommand::UpsertArtifact { artifact: row })
                .map_err(|error| BrowserError::Host(error.to_string()))?;
            self.update_timeline_status_by_id(
                projections,
                task_id,
                &projection_id,
                ARTIFACT_STATUS_INACCESSIBLE,
            )?;
        }
        drop(runtime);
        if !found {
            // A missing projection is repaired from the durable Product fact;
            // rebuild will derive `inaccessible` from the absent file.
            self.rebuild_browser_projections(task_id)?;
        }
        Ok(())
    }
}

impl BrowserArtifactSink for DesktopBrowserArtifactSink {
    fn record_screenshot(
        &self,
        scope: &BrowserScope,
        session: &str,
        turn: &str,
        bytes: &[u8],
        cancellation: &BrowserCancellation,
    ) -> Result<String, BrowserError> {
        let _commit = self
            .commit_lock
            .lock()
            .expect("browser artifact commit lock");
        if bytes.is_empty() {
            return Err(BrowserError::Host("Screenshot is empty".into()));
        }
        if cancellation.is_cancelled() {
            return Err(BrowserError::Cancelled);
        }
        let content_hash = digest_hex(&bytes);
        let root = self.artifact_root(scope);
        fs::create_dir_all(&root)
            .map_err(|_| BrowserError::Host("failed to prepare browser artifact storage".into()))?;
        let base_resource_ref = format!(
            "resource://browser/{}/{}/{}.png",
            Self::project_digest(scope.project_id.as_str()),
            Self::task_digest(scope.task_id.as_str()),
            content_hash
        );
        let final_path = root.join(format!("{content_hash}.png"));
        let project_digest = Self::project_digest(scope.project_id.as_str());
        let client = self
            .authority
            .client()
            .map_err(|error| BrowserError::Host(error.to_string()))?;
        let mut recovering = false;
        if let Some(existing) = client
            .products()
            .find_artifact_by_content_hash(&scope.task_id, &content_hash)
            .map_err(|error| BrowserError::Host(error.to_string()))?
        {
            let existing_ref_is_usable = existing
                .resource_ref
                .as_deref()
                .is_some_and(|value| Self::resource_ref_matches_scope(value, scope, &content_hash));
            let existing_is_materialized =
                existing.materialization == ArtifactMaterializationStatus::Materialized;
            let existing_path =
                self.artifact_path(scope, &content_hash, existing.resource_ref.as_deref());
            if !Self::materialized_matches(&existing_path, &content_hash)
                || !existing_ref_is_usable
                || !existing_is_materialized
            {
                self.mark_existing_inaccessible(&scope.task_id, &existing.id)?;
                recovering = true;
            } else {
                let runtime = self.authority.shared_runtime();
                let projections = runtime.inner().projections();
                let projection_id = format!("browser:{}", existing.id.as_str());
                let needs_repair = projections
                    .list_artifacts_for_task(&scope.task_id)
                    .iter()
                    .find(|row| row.id == projection_id)
                    .is_none_or(|row| {
                        row.status == ARTIFACT_STATUS_INACCESSIBLE
                            || row.status == ARTIFACT_STATUS_EXPIRED
                    })
                    || !projections
                        .list_for_task(&scope.task_id)
                        .iter()
                        .any(|event| event.id.as_str() == projection_id);
                drop(runtime);
                if needs_repair {
                    self.rebuild_browser_projections(&scope.task_id)?;
                }
                return Ok(existing
                    .resource_ref
                    .expect("usable browser artifact must have a resource reference"));
            }
        }

        // Keep recovery identity deterministic across independently-created
        // sinks and process retries. Product command idempotency then
        // collapses concurrent recovery attempts to one durable row/event.
        let artifact_suffix = if recovering { "-recovery" } else { "" };
        // Recovery identities are intentionally distinct when the durable
        // Product row exists but its materialization is unavailable. The
        // resource reference still names the canonical hash file; adding a
        // recovery suffix here would point to a file that is never created.
        let resource_ref = base_resource_ref;
        let command_id = format!(
            "browser-screenshot:{}:{}:{content_hash}{artifact_suffix}",
            project_digest,
            scope.task_id.as_str()
        );

        let temp = root.join(format!(
            ".{content_hash}.tmp-{}-{}",
            std::process::id(),
            NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        if let Err(_error) = fs::write(&temp, bytes) {
            let _ = fs::remove_file(&temp);
            return Err(BrowserError::Host(
                "failed to write browser artifact".into(),
            ));
        }
        let written_hash = match fs::read(&temp) {
            Ok(written) => digest_hex(&written),
            Err(_) => {
                let _ = fs::remove_file(&temp);
                return Err(BrowserError::Host(
                    "failed to verify browser artifact".into(),
                ));
            }
        };
        if written_hash != content_hash {
            let _ = fs::remove_file(&temp);
            return Err(BrowserError::Host(
                "browser artifact hash verification failed".into(),
            ));
        }
        let mut materialized_here = false;
        if Self::materialized_matches(&final_path, &content_hash) {
            let _ = fs::remove_file(&temp);
        } else {
            // A same-name file with a different digest is corrupt material
            // managed state. Remove only that exact file before promoting the
            // verified temporary file; never accept it as this capture.
            if final_path.exists() {
                if let Err(_error) = fs::remove_file(&final_path) {
                    let _ = fs::remove_file(&temp);
                    return Err(BrowserError::Host(
                        "failed to replace browser artifact".into(),
                    ));
                }
            }
            if let Err(_error) = fs::rename(&temp, &final_path) {
                let _ = fs::remove_file(&temp);
                return Err(BrowserError::Host(
                    "failed to commit browser artifact".into(),
                ));
            }
            materialized_here = true;
        }
        if cancellation.is_cancelled() {
            if materialized_here {
                let _ = fs::remove_file(&final_path);
            }
            return Err(BrowserError::Cancelled);
        }
        let artifact_id = ArtifactId::new(format!(
            "browser-screenshot-{}-{}-{content_hash}{artifact_suffix}",
            project_digest,
            scope.task_id.as_str()
        ))
        .map_err(|error| BrowserError::Host(error.to_string()))?;
        let result = client
            .products()
            .record_browser_screenshot(lilia_service::BrowserScreenshotInput {
                task_id: scope.task_id.clone(),
                project_id: scope.project_id.clone(),
                agent_session: AgentSessionRef::new(session.to_owned())
                    .map_err(|error| BrowserError::Host(error.to_string()))?,
                artifact_id,
                artifact_ref: format!("browser.screenshot:{content_hash}"),
                resource_ref: resource_ref.clone(),
                content_hash,
                size_bytes: bytes.len() as u64,
                command_id: command_id.clone(),
            })
            .map_err(|error| {
                if materialized_here {
                    let _ = fs::remove_file(&final_path);
                }
                BrowserError::Host(error.to_string())
            })?;
        let artifact = match result.value {
            ProductEntity::Artifact(artifact) => artifact,
            _ => {
                if materialized_here {
                    let _ = fs::remove_file(&final_path);
                }
                return Err(BrowserError::Host(
                    "browser artifact command returned wrong entity".into(),
                ));
            }
        };
        if !result.duplicate {
            let agent_session = AgentSessionRef::new(session.to_owned())
                .map_err(|error| BrowserError::Host(error.to_string()))?;
            let sequence = self.next_projection_sequence(&agent_session);
            self.project(
                scope,
                session,
                turn,
                &command_id,
                &artifact,
                sequence,
                "available",
            )
            .map_err(|error| {
                // Keep a materialized file when the Product fact exists: a
                // later rebuild can safely repair the projection.
                error
            })?;
        } else if recovering {
            // A concurrent/retried recovery may have reused the durable
            // command result. Rebuild so its projection reflects the file we
            // just verified and materialized.
            self.rebuild_browser_projections(&scope.task_id)?;
        }
        Ok(artifact.resource_ref.unwrap_or(resource_ref))
    }
}

fn digest_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lilia_contracts::{AgentSessionRef, PageRequest, ProjectId, TaskId};
    use tempfile::tempdir;

    #[test]
    fn screenshot_is_product_artifact_and_timeline_event_and_deduplicates() {
        let home = tempdir().unwrap();
        let authority = ServiceAuthority::bootstrap_in_memory_named(
            "test:browser-artifacts",
            "browser-artifacts",
        )
        .unwrap();
        let client = authority.client().unwrap();
        let project_id = ProjectId::new("browser-project").unwrap();
        let task_id = TaskId::new("browser-task").unwrap();
        client
            .products()
            .create_project(project_id.clone(), "Browser project")
            .unwrap();
        client
            .products()
            .create_task(task_id.clone(), Some(project_id.clone()), "Capture")
            .unwrap();
        let scope = BrowserScope {
            project_id,
            task_id: task_id.clone(),
            tab_id: "tab".into(),
        };
        let sink = DesktopBrowserArtifactSink::new(authority.clone(), home.path());
        let cancellation = BrowserCancellation::default();
        let first = sink
            .record_screenshot(&scope, "session", "turn-1", b"png bytes", &cancellation)
            .unwrap();
        assert!(first.starts_with("resource://browser/"));
        let artifacts = client
            .products()
            .list_entities(lilia_contracts::ProductEntityKind::Artifact)
            .unwrap();
        assert_eq!(artifacts.len(), 1);
        let events = client.product_events(&PageRequest::default()).unwrap();
        assert_eq!(events.items[0].action, "browser_screenshot_captured");
        assert!(events.items[0]
            .command_id
            .starts_with("browser-screenshot:"));
        let timeline = authority.projection_timeline_for_task(&task_id);
        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].kind, "artifact");
        assert_eq!(
            authority
                .shared_runtime()
                .inner()
                .product_artifacts_for_task(&task_id)
                .len(),
            1
        );

        let runtime = authority.shared_runtime();
        runtime
            .inner()
            .projections()
            .clear_session(&AgentSessionRef::new("session").unwrap())
            .unwrap();
        assert!(authority.projection_timeline_for_task(&task_id).is_empty());
        let duplicate = sink
            .record_screenshot(&scope, "session", "turn-2", b"png bytes", &cancellation)
            .unwrap();
        assert_eq!(duplicate, first);
        assert_eq!(
            client
                .products()
                .list_entities(lilia_contracts::ProductEntityKind::Artifact)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(authority.projection_timeline_for_task(&task_id).len(), 1);
        sink.cleanup_browser_artifacts_for_task(
            &task_id,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            &ArtifactRetentionPolicy::default(),
        )
        .unwrap();
        assert!(sink
            .artifact_root(&scope)
            .join(format!("{}.png", digest_hex(b"png bytes")))
            .exists());
        std::fs::remove_file(
            sink.artifact_root(&scope)
                .join(format!("{}.png", digest_hex(b"png bytes"))),
        )
        .unwrap();
        sink.cleanup_browser_artifacts_for_task(
            &task_id,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            &ArtifactRetentionPolicy::default(),
        )
        .unwrap();
        assert_eq!(
            authority.projection_timeline_for_task(&task_id)[0].status,
            ARTIFACT_STATUS_INACCESSIBLE
        );
    }

    #[test]
    fn missing_duplicate_is_marked_inaccessible_and_captured_as_new_artifact() {
        let home = tempdir().unwrap();
        let authority = ServiceAuthority::bootstrap_in_memory_named(
            "test:browser-artifact-recovery",
            "browser-artifact-recovery",
        )
        .unwrap();
        let client = authority.client().unwrap();
        let project_id = ProjectId::new("recovery-project").unwrap();
        let task_id = TaskId::new("recovery-task").unwrap();
        client
            .products()
            .create_project(project_id.clone(), "Recovery")
            .unwrap();
        client
            .products()
            .create_task(task_id.clone(), Some(project_id.clone()), "Capture")
            .unwrap();
        let scope = BrowserScope {
            project_id,
            task_id: task_id.clone(),
            tab_id: "tab".into(),
        };
        let sink = DesktopBrowserArtifactSink::new(authority.clone(), home.path());
        let cancellation = BrowserCancellation::default();
        let first = sink
            .record_screenshot(&scope, "session", "turn-1", b"same png", &cancellation)
            .unwrap();
        let path = sink
            .artifact_root(&scope)
            .join(format!("{}.png", digest_hex(b"same png")));
        fs::remove_file(path).unwrap();

        let recovered = sink
            .record_screenshot(&scope, "session", "turn-2", b"same png", &cancellation)
            .unwrap();
        // The canonical hash resource is recreated in place; recovery gets a
        // new Product identity but does not invent a reference to a
        // non-existent suffixed file.
        assert_eq!(recovered, first);
        assert_eq!(
            client
                .products()
                .list_entities(ProductEntityKind::Artifact)
                .unwrap()
                .len(),
            2
        );
        let rows = authority
            .shared_runtime()
            .inner()
            .projections()
            .list_artifacts_for_task(&task_id);
        assert_eq!(rows.len(), 2);
        assert!(rows
            .iter()
            .any(|row| row.status == ARTIFACT_STATUS_INACCESSIBLE));
        assert!(rows
            .iter()
            .any(|row| row.status == ARTIFACT_STATUS_AVAILABLE));

        // A separately-constructed sink must use the same recovery identity,
        // rather than creating another Product row for the same content.
        let second_sink = DesktopBrowserArtifactSink::new(authority.clone(), home.path());
        let recovered_again = second_sink
            .record_screenshot(&scope, "session", "turn-3", b"same png", &cancellation)
            .unwrap();
        assert_eq!(recovered_again, recovered);
        assert_eq!(
            client
                .products()
                .list_entities(ProductEntityKind::Artifact)
                .unwrap()
                .len(),
            2
        );

        sink.cleanup_browser_artifacts_for_task(
            &task_id,
            u64::MAX,
            &ArtifactRetentionPolicy::default(),
        )
        .unwrap();
        let statuses = authority
            .projection_timeline_for_task(&task_id)
            .into_iter()
            .map(|event| event.status)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            statuses,
            BTreeSet::from([
                ARTIFACT_STATUS_EXPIRED.to_owned(),
                ARTIFACT_STATUS_INACCESSIBLE.to_owned(),
            ])
        );
    }

    #[test]
    fn pinned_projection_survives_rebuild_and_protects_materialization() {
        let home = tempdir().unwrap();
        let authority = ServiceAuthority::bootstrap_in_memory_named(
            "test:browser-artifact-pinned",
            "browser-artifact-pinned",
        )
        .unwrap();
        let client = authority.client().unwrap();
        let project_id = ProjectId::new("pinned-project").unwrap();
        let task_id = TaskId::new("pinned-task").unwrap();
        client
            .products()
            .create_project(project_id.clone(), "Pinned")
            .unwrap();
        client
            .products()
            .create_task(task_id.clone(), Some(project_id.clone()), "Capture")
            .unwrap();
        let scope = BrowserScope {
            project_id,
            task_id: task_id.clone(),
            tab_id: "tab".into(),
        };
        let sink = DesktopBrowserArtifactSink::new(authority.clone(), home.path());
        sink.record_screenshot(
            &scope,
            "session",
            "turn",
            b"shared png",
            &BrowserCancellation::default(),
        )
        .unwrap();
        let runtime = authority.shared_runtime();
        let projections = runtime.inner().projections();
        let mut pinned = projections.list_artifacts_for_task(&task_id)[0].clone();
        pinned.status = ARTIFACT_STATUS_PINNED.into();
        projections
            .apply(TimelineProjectionCommand::UpsertArtifact {
                artifact: pinned.clone(),
            })
            .unwrap();
        drop(runtime);

        sink.cleanup_browser_artifacts_for_task(
            &task_id,
            u64::MAX,
            &ArtifactRetentionPolicy::default(),
        )
        .unwrap();
        assert!(sink
            .artifact_root(&scope)
            .join(format!("{}.png", digest_hex(b"shared png")))
            .is_file());
        let rows = authority
            .shared_runtime()
            .inner()
            .projections()
            .list_artifacts_for_task(&task_id);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, ARTIFACT_STATUS_PINNED);
    }

    #[test]
    fn concurrent_same_hash_capture_creates_one_product_fact() {
        let home = tempdir().unwrap();
        let authority = ServiceAuthority::bootstrap_in_memory_named(
            "test:browser-artifact-concurrent",
            "browser-artifact-concurrent",
        )
        .unwrap();
        let client = authority.client().unwrap();
        let project_id = ProjectId::new("concurrent-project").unwrap();
        let task_id = TaskId::new("concurrent-task").unwrap();
        client
            .products()
            .create_project(project_id.clone(), "Concurrent")
            .unwrap();
        client
            .products()
            .create_task(task_id.clone(), Some(project_id.clone()), "Capture")
            .unwrap();
        let scope = BrowserScope {
            project_id,
            task_id: task_id.clone(),
            tab_id: "tab".into(),
        };
        let sink = std::sync::Arc::new(DesktopBrowserArtifactSink::new(
            authority.clone(),
            home.path(),
        ));
        let mut workers = Vec::new();
        for index in 0..8 {
            let sink = sink.clone();
            let scope = scope.clone();
            workers.push(std::thread::spawn(move || {
                sink.record_screenshot(
                    &scope,
                    "session",
                    &format!("turn-{index}"),
                    b"same concurrent png",
                    &BrowserCancellation::default(),
                )
                .unwrap()
            }));
        }
        let refs = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        assert!(refs.windows(2).all(|pair| pair[0] == pair[1]));
        assert_eq!(
            client
                .products()
                .list_entities(ProductEntityKind::Artifact)
                .unwrap()
                .len(),
            1
        );
        let events = client
            .products()
            .product_events(&PageRequest::default())
            .unwrap();
        assert_eq!(
            events
                .items
                .iter()
                .filter(|event| event.action == "browser_screenshot_captured")
                .count(),
            1
        );
        assert_eq!(
            authority
                .shared_runtime()
                .inner()
                .projections()
                .list_artifacts_for_task(&task_id)
                .len(),
            1
        );
    }

    #[test]
    fn corrupt_canonical_file_is_replaced_before_recording_recovery() {
        let home = tempdir().unwrap();
        let authority = ServiceAuthority::bootstrap_in_memory_named(
            "test:browser-artifact-corrupt",
            "browser-artifact-corrupt",
        )
        .unwrap();
        let client = authority.client().unwrap();
        let project_id = ProjectId::new("corrupt-project").unwrap();
        let task_id = TaskId::new("corrupt-task").unwrap();
        client
            .products()
            .create_project(project_id.clone(), "Corrupt")
            .unwrap();
        client
            .products()
            .create_task(task_id.clone(), Some(project_id.clone()), "Capture")
            .unwrap();
        let scope = BrowserScope {
            project_id,
            task_id,
            tab_id: "tab".into(),
        };
        let sink = DesktopBrowserArtifactSink::new(authority, home.path());
        let cancellation = BrowserCancellation::default();
        sink.record_screenshot(&scope, "session", "turn-1", b"png", &cancellation)
            .unwrap();
        let path = sink
            .artifact_root(&scope)
            .join(format!("{}.png", digest_hex(b"png")));
        fs::write(&path, b"corrupt").unwrap();
        sink.record_screenshot(&scope, "session", "turn-2", b"png", &cancellation)
            .unwrap();
        assert!(DesktopBrowserArtifactSink::materialized_matches(
            &path,
            &digest_hex(b"png")
        ));
    }

    #[test]
    fn sqlite_product_and_projection_reopen_rebuilds_browser_artifact() {
        let home = tempdir().unwrap();
        let project_id = ProjectId::new("reopen-project").unwrap();
        let task_id = TaskId::new("reopen-task").unwrap();
        {
            let authority = ServiceAuthority::bootstrap_with_home(home.path()).unwrap();
            let client = authority.client().unwrap();
            client
                .products()
                .create_project(project_id.clone(), "Reopen")
                .unwrap();
            client
                .products()
                .create_task(task_id.clone(), Some(project_id.clone()), "Capture")
                .unwrap();
            let sink = DesktopBrowserArtifactSink::new(authority, home.path());
            sink.record_screenshot(
                &BrowserScope {
                    project_id: project_id.clone(),
                    task_id: task_id.clone(),
                    tab_id: "tab".into(),
                },
                "session",
                "turn",
                b"persisted png",
                &BrowserCancellation::default(),
            )
            .unwrap();
        }
        let authority = ServiceAuthority::bootstrap_with_home(home.path()).unwrap();
        let client = authority.client().unwrap();
        assert_eq!(
            client
                .products()
                .list_entities(ProductEntityKind::Artifact)
                .unwrap()
                .len(),
            1
        );
        let session = AgentSessionRef::new("session").unwrap();
        authority
            .shared_runtime()
            .inner()
            .projections()
            .clear_session(&session)
            .unwrap();
        let sink = DesktopBrowserArtifactSink::new(authority.clone(), home.path());
        assert_eq!(sink.rebuild_browser_projections(&task_id).unwrap(), 1);
        assert_eq!(authority.projection_timeline_for_task(&task_id).len(), 1);
        assert_eq!(
            authority
                .shared_runtime()
                .inner()
                .projections()
                .list_artifacts_for_task(&task_id)
                .len(),
            1
        );
    }
}
