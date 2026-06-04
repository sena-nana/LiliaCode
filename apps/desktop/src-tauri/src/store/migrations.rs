use rusqlite::Connection;

pub(super) const RESET_BASELINE_SCHEMA_VERSION: i64 = 3;

pub(super) struct SchemaMigration {
    pub version: i64,
    pub name: &'static str,
    pub apply: fn(&Connection) -> Result<(), String>,
}

pub(super) const SCHEMA_MIGRATIONS: &[SchemaMigration] = &[
    SchemaMigration {
        version: 4,
        name: "todo_guides",
        apply: migrate_todo_guides,
    },
    SchemaMigration {
        version: 5,
        name: "todo_attachments",
        apply: migrate_todo_attachments,
    },
    SchemaMigration {
        version: 6,
        name: "task_list_indexes",
        apply: migrate_task_list_indexes,
    },
];

fn migrate_todo_guides(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE task_todos_next (
          id           TEXT PRIMARY KEY,
          task_id      TEXT NOT NULL,
          text         TEXT NOT NULL,
          done         INTEGER NOT NULL DEFAULT 0,
          "order"      INTEGER NOT NULL,
          source       TEXT NOT NULL CHECK (source IN ('lilia','agent')),
          priority     TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('high','normal','low')),
          guide_status TEXT CHECK (guide_status IS NULL OR guide_status IN ('pending','queued','sent')),
          created_at   INTEGER NOT NULL,
          updated_at   INTEGER NOT NULL
        );

        INSERT INTO task_todos_next
          (id, task_id, text, done, "order", source, priority, guide_status, created_at, updated_at)
        SELECT
          id,
          task_id,
          text,
          done,
          "order",
          CASE source WHEN 'user' THEN 'lilia' ELSE source END,
          'normal',
          CASE source WHEN 'agent' THEN NULL ELSE 'pending' END,
          created_at,
          updated_at
        FROM task_todos;

        DROP TABLE task_todos;
        ALTER TABLE task_todos_next RENAME TO task_todos;
        CREATE INDEX idx_task_todos_task_id_order
          ON task_todos(task_id, "order");
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 todo_guides 失败：{e}"))
}

fn migrate_todo_attachments(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        ALTER TABLE task_todos
          ADD COLUMN attachments_json TEXT NOT NULL DEFAULT '[]';
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 todo_attachments 失败：{e}"))
}

fn migrate_task_list_indexes(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_tasks_project_archived_order
          ON tasks(project_id, archived, pinned DESC, sort_order ASC);
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 task_list_indexes 失败：{e}"))
}

pub(super) fn ensure_schema_with_migrations(
    conn: &mut Connection,
    target_version: i64,
    migrations: &[SchemaMigration],
) -> Result<(), String> {
    let current: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| format!("lilia-store: 读取 user_version 失败：{e}"))?;

    if current < RESET_BASELINE_SCHEMA_VERSION || current > target_version {
        super::schema::reset_development_schema(conn)?;
    }

    let mut version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| format!("lilia-store: 读取重置后 user_version 失败：{e}"))?;

    for migration in migrations {
        if migration.version <= version {
            continue;
        }
        if migration.version != version + 1 {
            return Err(format!(
                "lilia-store: schema migration {} 版本不连续，当前 {version}",
                migration.name
            ));
        }
        (migration.apply)(conn)?;
        conn.execute_batch(&format!("PRAGMA user_version = {};", migration.version))
            .map_err(|e| format!("lilia-store: 写 {} 版本失败：{e}", migration.name))?;
        version = migration.version;
    }

    Ok(())
}
