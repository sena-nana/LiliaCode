use std::collections::HashSet;

use rusqlite::{params, OptionalExtension};
use tauri::State;
use uuid::Uuid;

use crate::store::LiliaStore;
use crate::util::now_millis;

use super::types::{MilestoneRow, ProjectRoadmapRow, TaskMilestoneLinkRow};

const MILESTONE_STATUSES: [&str; 4] = ["upcoming", "in-progress", "done", "abandoned"];

fn validate_status(status: &str) -> Result<(), String> {
    if MILESTONE_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(format!("milestone_update: 无效状态：{status}"))
    }
}

fn row_to_milestone(row: &rusqlite::Row<'_>) -> rusqlite::Result<MilestoneRow> {
    Ok(MilestoneRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        status: row.get(4)?,
        due_date: row.get(5)?,
        order: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn load_milestone_project_id(
    conn: &rusqlite::Connection,
    milestone_id: &str,
) -> Result<String, String> {
    conn.query_row(
        "SELECT project_id FROM milestones WHERE id = ?1",
        params![milestone_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| format!("milestone_set_tasks: 查询 milestone 失败：{e}"))?
    .ok_or_else(|| "milestone_set_tasks: milestone 不存在".to_string())
}

fn set_milestone_tasks_core(
    conn: &mut rusqlite::Connection,
    milestone_id: &str,
    task_ids: Vec<String>,
) -> Result<Vec<TaskMilestoneLinkRow>, String> {
    let project_id = load_milestone_project_id(conn, milestone_id)?;
    let mut seen = HashSet::new();
    let task_ids: Vec<String> = task_ids
        .into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect();

    for task_id in &task_ids {
        let valid = conn
            .query_row(
                "SELECT 1 FROM tasks WHERE id = ?1 AND project_id = ?2 AND archived = 0",
                params![task_id.as_str(), project_id.as_str()],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| format!("milestone_set_tasks: 查询任务失败：{e}"))?
            .is_some();
        if !valid {
            return Err(format!(
                "milestone_set_tasks: 任务不属于当前项目：{task_id}"
            ));
        }
    }

    let tx = conn
        .transaction()
        .map_err(|e| format!("milestone_set_tasks: 开启事务失败：{e}"))?;
    tx.execute(
        "DELETE FROM task_milestone_links WHERE milestone_id = ?1",
        params![milestone_id],
    )
    .map_err(|e| format!("milestone_set_tasks: 清理旧关联失败：{e}"))?;
    for task_id in &task_ids {
        tx.execute(
            "INSERT INTO task_milestone_links (task_id, milestone_id) VALUES (?1, ?2)",
            params![task_id.as_str(), milestone_id],
        )
        .map_err(|e| format!("milestone_set_tasks: 写入关联失败：{e}"))?;
    }
    tx.commit()
        .map_err(|e| format!("milestone_set_tasks: 提交事务失败：{e}"))?;

    Ok(task_ids
        .into_iter()
        .map(|task_id| TaskMilestoneLinkRow {
            task_id,
            milestone_id: milestone_id.to_string(),
        })
        .collect())
}

#[tauri::command]
pub fn milestone_list(
    project_id: String,
    store: State<'_, LiliaStore>,
) -> Result<ProjectRoadmapRow, String> {
    let conn = store.conn()?;
    let mut milestone_stmt = conn
        .prepare(
            r#"SELECT id, project_id, title, description, status, due_date, sort_order, created_at
               FROM milestones
               WHERE project_id = ?1
               ORDER BY sort_order ASC, created_at ASC"#,
        )
        .map_err(|e| format!("milestone_list: prepare milestones 失败：{e}"))?;
    let milestone_rows = milestone_stmt
        .query_map(params![project_id.as_str()], row_to_milestone)
        .map_err(|e| format!("milestone_list: query milestones 失败：{e}"))?;
    let mut milestones = Vec::new();
    for row in milestone_rows {
        milestones.push(row.map_err(|e| format!("milestone_list: milestone row 失败：{e}"))?);
    }

    let mut link_stmt = conn
        .prepare(
            r#"SELECT l.task_id, l.milestone_id
               FROM task_milestone_links l
               INNER JOIN milestones m ON m.id = l.milestone_id
               INNER JOIN tasks t ON t.id = l.task_id
               WHERE m.project_id = ?1 AND t.project_id = ?1 AND t.archived = 0
               ORDER BY m.sort_order ASC, t.sort_order ASC, t.created_at ASC"#,
        )
        .map_err(|e| format!("milestone_list: prepare links 失败：{e}"))?;
    let link_rows = link_stmt
        .query_map(params![project_id.as_str()], |row| {
            Ok(TaskMilestoneLinkRow {
                task_id: row.get(0)?,
                milestone_id: row.get(1)?,
            })
        })
        .map_err(|e| format!("milestone_list: query links 失败：{e}"))?;
    let mut links = Vec::new();
    for row in link_rows {
        links.push(row.map_err(|e| format!("milestone_list: link row 失败：{e}"))?);
    }

    Ok(ProjectRoadmapRow { milestones, links })
}

#[tauri::command]
pub fn milestone_create(
    project_id: String,
    title: String,
    store: State<'_, LiliaStore>,
) -> Result<MilestoneRow, String> {
    let conn = store.conn()?;
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err("milestone_create: 标题不能为空".to_string());
    }
    let project_exists = conn
        .query_row(
            "SELECT 1 FROM projects WHERE id = ?1",
            params![project_id.as_str()],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| format!("milestone_create: 查询项目失败：{e}"))?
        .is_some();
    if !project_exists {
        return Err("milestone_create: 项目不存在".to_string());
    }
    let sort_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM milestones WHERE project_id = ?1",
            params![project_id.as_str()],
            |row| row.get(0),
        )
        .map_err(|e| format!("milestone_create: sort_order 失败：{e}"))?;
    let id = Uuid::new_v4().to_string();
    let now = now_millis();
    conn.execute(
        r#"INSERT INTO milestones
           (id, project_id, title, description, status, due_date, sort_order, created_at)
           VALUES (?1, ?2, ?3, '', 'upcoming', NULL, ?4, ?5)"#,
        params![id.as_str(), project_id.as_str(), trimmed, sort_order, now],
    )
    .map_err(|e| format!("milestone_create: insert 失败：{e}"))?;

    Ok(MilestoneRow {
        id,
        project_id,
        title: trimmed.to_string(),
        description: String::new(),
        status: "upcoming".to_string(),
        due_date: None,
        order: sort_order,
        created_at: now,
    })
}

#[tauri::command]
pub fn milestone_update(
    id: String,
    title: Option<String>,
    status: Option<String>,
    store: State<'_, LiliaStore>,
) -> Result<(), String> {
    if title.is_none() && status.is_none() {
        return Ok(());
    }
    let conn = store.conn()?;
    let exists = conn
        .query_row(
            "SELECT 1 FROM milestones WHERE id = ?1",
            params![id.as_str()],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| format!("milestone_update: 查询 milestone 失败：{e}"))?
        .is_some();
    if !exists {
        return Err("milestone_update: milestone 不存在".to_string());
    }

    if let Some(next_title) = title {
        let trimmed = next_title.trim();
        if trimmed.is_empty() {
            return Err("milestone_update: 标题不能为空".to_string());
        }
        conn.execute(
            "UPDATE milestones SET title = ?1 WHERE id = ?2",
            params![trimmed, id.as_str()],
        )
        .map_err(|e| format!("milestone_update(title): {e}"))?;
    }

    if let Some(next_status) = status {
        validate_status(&next_status)?;
        conn.execute(
            "UPDATE milestones SET status = ?1 WHERE id = ?2",
            params![next_status, id.as_str()],
        )
        .map_err(|e| format!("milestone_update(status): {e}"))?;
    }

    Ok(())
}

#[tauri::command]
pub fn milestone_set_tasks(
    milestone_id: String,
    task_ids: Vec<String>,
    store: State<'_, LiliaStore>,
) -> Result<Vec<TaskMilestoneLinkRow>, String> {
    let mut conn = store.conn()?;
    set_milestone_tasks_core(&mut conn, &milestone_id, task_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_schema(conn: &Connection) {
        conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE projects (
              id         TEXT PRIMARY KEY,
              name       TEXT NOT NULL,
              cwd        TEXT,
              created_at INTEGER NOT NULL,
              sort_order INTEGER NOT NULL DEFAULT 0,
              pinned     INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE tasks (
              id          TEXT PRIMARY KEY,
              project_id  TEXT,
              session_id  TEXT NOT NULL,
              title       TEXT NOT NULL,
              title_source TEXT NOT NULL DEFAULT 'auto'
                            CHECK (title_source IN ('auto','manual')),
              status      TEXT NOT NULL DEFAULT 'waiting'
                            CHECK (status IN
                              ('draft','waiting','running','blocked','done','cancelled')),
              created_at  INTEGER NOT NULL,
              parent_id   TEXT,
              archived    INTEGER NOT NULL DEFAULT 0,
              sort_order  INTEGER NOT NULL DEFAULT 0,
              pinned      INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE milestones (
              id          TEXT PRIMARY KEY,
              project_id  TEXT NOT NULL,
              title       TEXT NOT NULL,
              description TEXT NOT NULL DEFAULT '',
              status      TEXT NOT NULL DEFAULT 'upcoming'
                          CHECK (status IN ('upcoming','in-progress','done','abandoned')),
              due_date    INTEGER,
              sort_order  INTEGER NOT NULL DEFAULT 0,
              created_at  INTEGER NOT NULL,
              FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            );
            CREATE TABLE task_milestone_links (
              task_id      TEXT NOT NULL,
              milestone_id TEXT NOT NULL,
              PRIMARY KEY (task_id, milestone_id),
              FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
              FOREIGN KEY (milestone_id) REFERENCES milestones(id) ON DELETE CASCADE
            );
            INSERT INTO projects (id, name, created_at) VALUES ('p1', 'P1', 1), ('p2', 'P2', 1);
            INSERT INTO tasks (id, project_id, session_id, title, status, created_at, sort_order)
              VALUES
                ('t1', 'p1', 't1', 'T1', 'waiting', 1, 0),
                ('t2', 'p1', 't2', 'T2', 'done', 2, 1),
                ('t3', 'p2', 't3', 'T3', 'waiting', 3, 0),
                ('t4', 'p1', 't4', 'T4', 'waiting', 4, 2);
            UPDATE tasks SET archived = 1 WHERE id = 't4';
            INSERT INTO milestones (id, project_id, title, status, sort_order, created_at)
              VALUES ('m1', 'p1', 'M1', 'upcoming', 0, 1);
            "#,
        )
        .unwrap();
    }

    #[test]
    fn validate_status_accepts_contract_values() {
        for status in MILESTONE_STATUSES {
            validate_status(status).unwrap();
        }
        assert!(validate_status("running").is_err());
    }

    #[test]
    fn set_tasks_replaces_links_for_same_project_unarchived_tasks() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_schema(&conn);
        conn.execute(
            "INSERT INTO task_milestone_links (task_id, milestone_id) VALUES ('t1', 'm1')",
            [],
        )
        .unwrap();

        let links = set_milestone_tasks_core(&mut conn, "m1", vec!["t2".to_string()]).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].task_id, "t2");

        let task_id: String = conn
            .query_row(
                "SELECT task_id FROM task_milestone_links WHERE milestone_id = 'm1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(task_id, "t2");
    }

    #[test]
    fn task_validation_rejects_cross_project_and_archived_tasks() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_schema(&conn);

        for task_id in ["t3", "t4"] {
            assert!(set_milestone_tasks_core(&mut conn, "m1", vec![task_id.to_string()]).is_err());
        }
    }
}
