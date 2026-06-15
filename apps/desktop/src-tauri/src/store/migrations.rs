use rusqlite::Connection;

pub(super) const RESET_BASELINE_SCHEMA_VERSION: i64 = 24;

pub(super) struct SchemaMigration {
    pub version: i64,
    pub name: &'static str,
    pub apply: fn(&Connection) -> Result<(), String>,
}

pub(super) const SCHEMA_MIGRATIONS: &[SchemaMigration] = &[];

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
