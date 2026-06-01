/*!
 * lilia-store：SQLite 持久化层。Todo / Memory / Roadmap 一期都复用这一个库。
 *
 * 设计要点：
 * - **home 解析**：env `LILIA_HOME` > `~/.lilia/.redirect` 文件内容 > 默认 `~/.lilia/`。
 *   解析后立刻确保 `config/ db/ cache/` 三个子目录存在，避免后续每处都自己 mkdir。
 * - **连接池**：r2d2 + r2d2_sqlite。SQLite 单 writer，但读路径走多 reader 仍然受益。
 *   WAL 模式 + busy_timeout 让并发读写不互踩。
 * - **schema baseline**：本次重置会把旧开发库清空并建成新基线；
 *   之后的 schema 变更继续在该基线后追加迁移。
 */

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;

const LILIA_HOME_ENV: &str = "LILIA_HOME";
const REDIRECT_FILE: &str = ".redirect";
const DEFAULT_HOME_SUBDIR: &str = ".lilia";

/// 解析 lilia home 目录。返回前确保目录及 config/db/cache 三个子目录存在。
///
/// 顺序：
/// 1. env `LILIA_HOME`（绝对路径）
/// 2. `~/.lilia/.redirect` 内的路径（用户把数据搬到别处时留个跳板，不强制移动）
/// 3. `~/.lilia/`
///
/// 任一步骤失败都向下兜底；最坏返回当前工作目录下的 `.lilia/` 让程序仍能跑起来。
pub fn resolve_lilia_home() -> PathBuf {
    let primary = resolve_home_candidate();
    let home = primary.unwrap_or_else(|| PathBuf::from(".lilia"));
    let _ = ensure_layout(&home);
    home
}

fn resolve_home_candidate() -> Option<PathBuf> {
    if let Ok(env_val) = env::var(LILIA_HOME_ENV) {
        let trimmed = env_val.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }

    let default_home = dirs::home_dir().map(|d| d.join(DEFAULT_HOME_SUBDIR));

    if let Some(home) = &default_home {
        let redirect = home.join(REDIRECT_FILE);
        if redirect.is_file() {
            if let Ok(raw) = fs::read_to_string(&redirect) {
                let target = raw.trim();
                if !target.is_empty() {
                    return Some(PathBuf::from(target));
                }
            }
        }
    }

    default_home
}

fn ensure_layout(home: &Path) -> std::io::Result<()> {
    fs::create_dir_all(home)?;
    for sub in ["config", "db", "cache"] {
        fs::create_dir_all(home.join(sub))?;
    }
    Ok(())
}

pub struct LiliaStore {
    pool: Pool<SqliteConnectionManager>,
}

impl LiliaStore {
    /// 打开（或新建）`<home>/db/lilia.db`，启 WAL + foreign_keys，再确保当前 schema。
    pub fn new(home: &Path) -> Result<Self, String> {
        ensure_layout(home).map_err(|e| format!("lilia-store: 准备目录失败：{e}"))?;
        let db_path = home.join("db").join("lilia.db");

        let manager = SqliteConnectionManager::file(&db_path).with_init(|conn| {
            // 每个 pooled 连接都要打开 foreign_keys；WAL/busy_timeout 是全库 PRAGMA。
            conn.execute_batch(
                "PRAGMA foreign_keys = ON;\
                 PRAGMA journal_mode = WAL;\
                 PRAGMA synchronous = NORMAL;\
                 PRAGMA busy_timeout = 5000;",
            )
        });
        let pool = Pool::builder()
            .max_size(8)
            .build(manager)
            .map_err(|e| format!("lilia-store: 建连接池失败：{e}"))?;

        {
            let mut conn = pool
                .get()
                .map_err(|e| format!("lilia-store: 取连接失败：{e}"))?;
            ensure_current_schema(&mut conn)?;
            reset_orphaned_queued_guides(&conn)?;
        }

        Ok(LiliaStore { pool })
    }

    pub fn conn(&self) -> Result<PooledConnection<SqliteConnectionManager>, String> {
        self.pool
            .get()
            .map_err(|e| format!("lilia-store: 取连接失败：{e}"))
    }
}

fn reset_orphaned_queued_guides(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE task_todos SET guide_status = 'pending' WHERE source = 'lilia' AND guide_status = 'queued'",
        [],
    )
    .map(|_| ())
    .map_err(|e| format!("lilia-store: 重置 queued 引导失败：{e}"))
}

const RESET_BASELINE_SCHEMA_VERSION: i64 = 3;

struct SchemaMigration {
    version: i64,
    name: &'static str,
    apply: fn(&Connection) -> Result<(), String>,
}

const SCHEMA_MIGRATIONS: &[SchemaMigration] = &[
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
];

/// baseline=3：`agent_timeline_events` 的 `order` 列拆成
/// `(turn_seq, intra_turn_order)`，排序按 turn 隔离。跨过这个版本会触发
/// reset，旧 dev 数据丢失。
fn ensure_current_schema(conn: &mut Connection) -> Result<(), String> {
    ensure_schema_with_migrations(conn, current_schema_version(), SCHEMA_MIGRATIONS)
}

fn current_schema_version() -> i64 {
    SCHEMA_MIGRATIONS
        .last()
        .map(|migration| migration.version)
        .unwrap_or(RESET_BASELINE_SCHEMA_VERSION)
}

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

fn ensure_schema_with_migrations(
    conn: &mut Connection,
    target_version: i64,
    migrations: &[SchemaMigration],
) -> Result<(), String> {
    let current: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| format!("lilia-store: 读取 user_version 失败：{e}"))?;

    if current < RESET_BASELINE_SCHEMA_VERSION || current > target_version {
        reset_development_schema(conn)?;
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

fn reset_development_schema(conn: &Connection) -> Result<(), String> {
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
        "PRAGMA user_version = {RESET_BASELINE_SCHEMA_VERSION};"
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

    #[test]
    fn old_development_database_is_rebuilt_from_current_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE projects (
              id         TEXT PRIMARY KEY,
              name       TEXT NOT NULL,
              cwd        TEXT,
              created_at INTEGER NOT NULL
            );
            INSERT INTO projects (id, name, cwd, created_at)
              VALUES ('old-project', '旧项目', NULL, 1);
            PRAGMA user_version = 6;
            "#,
        )
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
    }

    fn add_future_project_label_migration(conn: &Connection) -> Result<(), String> {
        conn.execute_batch("ALTER TABLE projects ADD COLUMN label TEXT;")
            .map_err(|e| format!("test migration: {e}"))
    }

    #[test]
    fn todo_guides_migration_maps_user_rows_to_lilia() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_current_schema(&conn).unwrap();
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
        create_current_schema(&conn).unwrap();
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
    fn reset_orphaned_queued_guides_marks_them_pending() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_current_schema(&conn).unwrap();
        conn.execute_batch(&format!(
            "PRAGMA user_version = {RESET_BASELINE_SCHEMA_VERSION};"
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
