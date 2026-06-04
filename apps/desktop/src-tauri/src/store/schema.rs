use rusqlite::Connection;

use super::migrations::{
    ensure_schema_with_migrations, RESET_BASELINE_SCHEMA_VERSION, SCHEMA_MIGRATIONS,
};
#[cfg(test)]
use super::migrations::SchemaMigration;

pub(super) fn reset_orphaned_queued_guides(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE task_todos SET guide_status = 'pending' WHERE source = 'lilia' AND guide_status = 'queued'",
        [],
    )
    .map(|_| ())
    .map_err(|e| format!("lilia-store: 重置 queued 引导失败：{e}"))
}

pub(super) fn ensure_current_schema(conn: &mut Connection) -> Result<(), String> {
    ensure_schema_with_migrations(conn, current_schema_version(), SCHEMA_MIGRATIONS)
}

fn current_schema_version() -> i64 {
    SCHEMA_MIGRATIONS
        .last()
        .map(|migration| migration.version)
        .unwrap_or(RESET_BASELINE_SCHEMA_VERSION)
}

pub(super) fn reset_development_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = OFF;

        DROP TABLE IF EXISTS agent_timeline_events;
        DROP TABLE IF EXISTS task_dependencies;
        DROP TABLE IF EXISTS tasks;
        DROP TABLE IF EXISTS projects;
        DROP TABLE IF EXISTS task_todos;

        PRAGMA foreign_keys = ON;
        "#,
    )
    .map_err(|e| format!("lilia-store: 清空旧开发库失败：{e}"))?;
    create_current_schema(conn)?;
    conn.execute_batch(&format!(
        "PRAGMA user_version = {};",
        current_schema_version()
    ))
    .map_err(|e| format!("lilia-store: 写 user_version 失败：{e}"))
}

fn create_current_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE task_todos (
          id           TEXT PRIMARY KEY,
          task_id      TEXT NOT NULL,
          text         TEXT NOT NULL,
          done         INTEGER NOT NULL DEFAULT 0,
          "order"      INTEGER NOT NULL,
          source       TEXT NOT NULL CHECK (source IN ('lilia','agent')),
          priority     TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('high','normal','low')),
          guide_status TEXT CHECK (guide_status IS NULL OR guide_status IN ('pending','queued','sent')),
          attachments_json TEXT NOT NULL DEFAULT '[]',
          created_at   INTEGER NOT NULL,
          updated_at   INTEGER NOT NULL
        );
        CREATE INDEX idx_task_todos_task_id_order
          ON task_todos(task_id, "order");

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
          status      TEXT NOT NULL DEFAULT 'waiting'
                        CHECK (status IN
                          ('draft','waiting','running','blocked','done','cancelled')),
          created_at  INTEGER NOT NULL,
          parent_id   TEXT,
          archived    INTEGER NOT NULL DEFAULT 0,
          sort_order  INTEGER NOT NULL DEFAULT 0,
          pinned      INTEGER NOT NULL DEFAULT 0,
          FOREIGN KEY (project_id) REFERENCES projects(id)
        );

        CREATE INDEX idx_tasks_project_id
          ON tasks(project_id);
        CREATE INDEX idx_tasks_archived
          ON tasks(archived);
        CREATE INDEX idx_tasks_project_archived_order
          ON tasks(project_id, archived, pinned DESC, sort_order ASC);

        CREATE TABLE task_dependencies (
          task_id       TEXT NOT NULL,
          depends_on_id TEXT NOT NULL,
          PRIMARY KEY (task_id, depends_on_id),
          FOREIGN KEY (task_id)       REFERENCES tasks(id) ON DELETE CASCADE,
          FOREIGN KEY (depends_on_id) REFERENCES tasks(id) ON DELETE CASCADE
        );

        CREATE TABLE agent_timeline_events (
          id                TEXT PRIMARY KEY,
          task_id           TEXT NOT NULL,
          turn_id           TEXT,
          backend           TEXT NOT NULL CHECK (backend IN ('claude','codex')),
          kind              TEXT NOT NULL,
          status            TEXT NOT NULL,
          title             TEXT NOT NULL,
          summary           TEXT,
          payload           TEXT NOT NULL,
          created_at        INTEGER NOT NULL,
          updated_at        INTEGER NOT NULL,
          turn_seq          INTEGER NOT NULL,
          intra_turn_order  INTEGER NOT NULL
        );

        CREATE INDEX idx_agent_timeline_events_task_id_turn
          ON agent_timeline_events(task_id, turn_seq, intra_turn_order);
        "#,
    )
    .map_err(|e| format!("lilia-store: 创建当前 schema 失败：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn create_reset_baseline_schema(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE task_todos (
              id           TEXT PRIMARY KEY,
              task_id      TEXT NOT NULL,
              text         TEXT NOT NULL,
              done         INTEGER NOT NULL DEFAULT 0,
              "order"      INTEGER NOT NULL,
              source       TEXT NOT NULL CHECK (source IN ('user','agent')),
              created_at   INTEGER NOT NULL,
              updated_at   INTEGER NOT NULL
            );
            CREATE INDEX idx_task_todos_task_id_order
              ON task_todos(task_id, "order");

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
              status      TEXT NOT NULL DEFAULT 'waiting'
                            CHECK (status IN
                              ('draft','waiting','running','blocked','done','cancelled')),
              created_at  INTEGER NOT NULL,
              parent_id   TEXT,
              archived    INTEGER NOT NULL DEFAULT 0,
              sort_order  INTEGER NOT NULL DEFAULT 0,
              pinned      INTEGER NOT NULL DEFAULT 0,
              FOREIGN KEY (project_id) REFERENCES projects(id)
            );

            CREATE INDEX idx_tasks_project_id
              ON tasks(project_id);
            CREATE INDEX idx_tasks_archived
              ON tasks(archived);
            CREATE INDEX idx_tasks_project_archived_order
              ON tasks(project_id, archived, pinned DESC, sort_order ASC);

            CREATE TABLE task_dependencies (
              task_id       TEXT NOT NULL,
              depends_on_id TEXT NOT NULL,
              PRIMARY KEY (task_id, depends_on_id),
              FOREIGN KEY (task_id)       REFERENCES tasks(id) ON DELETE CASCADE,
              FOREIGN KEY (depends_on_id) REFERENCES tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE agent_timeline_events (
              id                TEXT PRIMARY KEY,
              task_id           TEXT NOT NULL,
              turn_id           TEXT,
              backend           TEXT NOT NULL CHECK (backend IN ('claude','codex')),
              kind              TEXT NOT NULL,
              status            TEXT NOT NULL,
              title             TEXT NOT NULL,
              summary           TEXT,
              payload           TEXT NOT NULL,
              created_at        INTEGER NOT NULL,
              updated_at        INTEGER NOT NULL,
              turn_seq          INTEGER NOT NULL,
              intra_turn_order  INTEGER NOT NULL
            );

            CREATE INDEX idx_agent_timeline_events_task_id_turn
              ON agent_timeline_events(task_id, turn_seq, intra_turn_order);
            "#,
        )
        .unwrap();
    }

    #[test]
    fn old_development_database_is_rebuilt_from_current_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!(
            r#"
            CREATE TABLE projects (
              id         TEXT PRIMARY KEY,
              name       TEXT NOT NULL,
              cwd        TEXT,
              created_at INTEGER NOT NULL
            );
            INSERT INTO projects (id, name, cwd, created_at)
              VALUES ('old-project', '旧项目', NULL, 1);
            PRAGMA user_version = {};
            "#,
            current_schema_version() + 1
        ))
        .unwrap();

        ensure_current_schema(&mut conn).unwrap();

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, current_schema_version());

        let project_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .unwrap();
        assert_eq!(project_count, 0);

        conn.execute(
            "INSERT INTO projects (id, name, cwd, created_at, sort_order, pinned) VALUES (?1, ?2, NULL, ?3, ?4, ?5)",
            params!["new-project", "新项目", 2, 0, 0],
        )
        .unwrap();

        conn.execute(
            r#"INSERT INTO task_todos
               (id, task_id, text, done, "order", source, priority, guide_status, attachments_json, created_at, updated_at)
               VALUES (?1, ?2, ?3, 0, 0, 'lilia', 'normal', 'pending', '[]', ?4, ?5)"#,
            params!["fresh-todo", "task-1", "当前 Todo", 2, 2],
        )
        .unwrap();
    }

    fn add_future_project_label_migration(conn: &Connection) -> Result<(), String> {
        conn.execute_batch("ALTER TABLE projects ADD COLUMN label TEXT;")
            .map_err(|e| format!("test migration: {e}"))
    }

    #[test]
    fn todo_guides_migration_maps_user_rows_to_lilia() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_reset_baseline_schema(&conn);
        conn.execute(
            r#"INSERT INTO task_todos
               (id, task_id, text, done, "order", source, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params!["todo-1", "task-1", "旧用户 Todo", 0, 0, "user", 1, 1],
        )
        .unwrap();
        conn.execute_batch(&format!(
            "PRAGMA user_version = {RESET_BASELINE_SCHEMA_VERSION};"
        ))
        .unwrap();

        ensure_current_schema(&mut conn).unwrap();

        let row: (String, String, Option<String>) = conn
            .query_row(
                "SELECT source, priority, guide_status FROM task_todos WHERE id = 'todo-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            row,
            (
                "lilia".to_string(),
                "normal".to_string(),
                Some("pending".to_string()),
            ),
        );
    }

    #[test]
    fn todo_attachments_migration_defaults_existing_rows() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_reset_baseline_schema(&conn);
        conn.execute(
            r#"INSERT INTO task_todos
               (id, task_id, text, done, "order", source, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params!["todo-attachments", "task-1", "旧 Todo", 0, 0, "user", 1, 1],
        )
        .unwrap();
        conn.execute_batch(&format!(
            "PRAGMA user_version = {RESET_BASELINE_SCHEMA_VERSION};"
        ))
        .unwrap();

        ensure_current_schema(&mut conn).unwrap();

        let attachments_json: String = conn
            .query_row(
                "SELECT attachments_json FROM task_todos WHERE id = 'todo-attachments'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(attachments_json, "[]");
    }

    #[test]
    fn task_list_indexes_migration_creates_composite_index() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_reset_baseline_schema(&conn);
        conn.execute_batch("PRAGMA user_version = 5;").unwrap();

        ensure_current_schema(&mut conn).unwrap();

        let index_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_tasks_project_archived_order'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(index_count, 1);
    }

    #[test]
    fn reset_orphaned_queued_guides_marks_them_pending() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_current_schema(&conn).unwrap();
        conn.execute_batch(&format!(
            "PRAGMA user_version = {};",
            current_schema_version()
        ))
        .unwrap();
        ensure_current_schema(&mut conn).unwrap();
        conn.execute(
            r#"INSERT INTO task_todos
               (id, task_id, text, done, "order", source, priority, guide_status, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, 'lilia', 'normal', 'queued', ?6, ?7)"#,
            params!["guide-1", "task-1", "排队引导", 0, 0, 1, 1],
        )
        .unwrap();

        reset_orphaned_queued_guides(&conn).unwrap();

        let status: String = conn
            .query_row(
                "SELECT guide_status FROM task_todos WHERE id = 'guide-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "pending");
    }

    #[test]
    fn future_migrations_continue_after_reset_baseline() {
        let mut conn = Connection::open_in_memory().unwrap();
        ensure_current_schema(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, cwd, created_at, sort_order, pinned) VALUES (?1, ?2, NULL, ?3, ?4, ?5)",
            params!["kept-project", "保留项目", 2, 0, 0],
        )
        .unwrap();

        ensure_schema_with_migrations(
            &mut conn,
            current_schema_version() + 1,
            &[SchemaMigration {
                version: current_schema_version() + 1,
                name: "test_add_project_label",
                apply: add_future_project_label_migration,
            }],
        )
        .unwrap();

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, current_schema_version() + 1);

        let project_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .unwrap();
        assert_eq!(project_count, 1);

        conn.execute(
            "UPDATE projects SET label = ?1 WHERE id = ?2",
            params!["future", "kept-project"],
        )
        .unwrap();
    }
}
