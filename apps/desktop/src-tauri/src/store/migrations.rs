use rusqlite::Connection;

pub(super) const RESET_BASELINE_SCHEMA_VERSION: i64 = 24;

pub(super) struct SchemaMigration {
    pub version: i64,
    pub name: &'static str,
    pub apply: fn(&Connection) -> Result<(), String>,
}

pub(super) const SCHEMA_MIGRATIONS: &[SchemaMigration] = &[SchemaMigration {
    version: 25,
    name: "memory_layer1",
    apply: create_memory_tables,
}];

fn create_memory_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS memories (
          id             TEXT PRIMARY KEY,
          scope          TEXT NOT NULL CHECK (scope IN ('user','project')),
          project_id     TEXT,
          title          TEXT NOT NULL,
          body           TEXT NOT NULL,
          tags_json      TEXT NOT NULL DEFAULT '[]',
          enabled        INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
          source_task_id TEXT,
          created_at     INTEGER NOT NULL,
          updated_at     INTEGER NOT NULL,
          CHECK (
            (scope = 'user' AND project_id IS NULL) OR
            (scope = 'project' AND project_id IS NOT NULL)
          ),
          FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_memories_scope_project
          ON memories(scope, project_id, enabled, updated_at DESC);

        CREATE TABLE IF NOT EXISTS memory_injection_states (
          task_id                TEXT PRIMARY KEY,
          enabled                INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
          last_injected_turn_seq INTEGER,
          updated_at             INTEGER NOT NULL,
          FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
        );
        "#,
    )
    .map_err(|e| format!("lilia-store: 创建 Memory schema 失败：{e}"))
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

    let current: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| format!("lilia-store: 读取重置后 user_version 失败：{e}"))?;

    for migration in migrations {
        if migration.version <= current || migration.version > target_version {
            continue;
        }
        (migration.apply)(conn)
            .map_err(|e| format!("lilia-store: migration {} failed: {e}", migration.name))?;
        conn.execute_batch(&format!("PRAGMA user_version = {};", migration.version))
            .map_err(|e| format!("lilia-store: 写 migration version 失败：{e}"))?;
    }

    Ok(())
}
