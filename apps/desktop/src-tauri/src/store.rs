/*!
 * lilia-store：SQLite 持久化层。Todo / Memory / Roadmap 一期都复用这一个库。
 *
 * 设计要点：
 * - **home 解析**：env `LILIA_HOME` > `~/.lilia/.redirect` 文件内容 > 默认 `~/.lilia/`。
 *   解析后立刻确保 `config/ db/ cache/` 三个子目录存在，避免后续每处都自己 mkdir。
 * - **连接池**：r2d2 + r2d2_sqlite。SQLite 单 writer，但读路径走多 reader 仍然受益。
 *   WAL 模式 + busy_timeout 让并发读写不互踩。
 * - **migrations**：用内置的 `PRAGMA user_version` 跟踪，避免引入第三方 migration 包。
 *   每个版本一个 closure，按 (version, fn) 顺序跑；当前版本号 = list 长度。
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
    // 1) env override
    if let Ok(env_val) = env::var(LILIA_HOME_ENV) {
        let trimmed = env_val.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }

    let default_home = dirs::home_dir().map(|d| d.join(DEFAULT_HOME_SUBDIR));

    // 2) ~/.lilia/.redirect → 路径
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

    // 3) 默认 ~/.lilia/
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
    /// 打开（或新建）`<home>/db/lilia.db`，启 WAL + foreign_keys，再跑 migrations。
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

        // 跑 migrations 用独立连接（用完即归还）。
        {
            let mut conn = pool
                .get()
                .map_err(|e| format!("lilia-store: 取连接失败：{e}"))?;
            run_migrations(&mut conn)?;
        }

        Ok(LiliaStore { pool })
    }

    pub fn conn(&self) -> Result<PooledConnection<SqliteConnectionManager>, String> {
        self.pool
            .get()
            .map_err(|e| format!("lilia-store: 取连接失败：{e}"))
    }
}

/// 每个 migration = 一个 closure；按下标顺序 = 目标 user_version。
/// 增加 schema 变更时在末尾追加一项，**禁止**改动已有 closure 的语义。
fn run_migrations(conn: &mut Connection) -> Result<(), String> {
    type Mig = fn(&Connection) -> Result<(), String>;
    let migrations: &[Mig] = &[
        // v1: TaskTodo 主表
        migration_v1_task_todos,
        // v2: projects + tasks + task_dependencies
        migration_v2_projects_tasks,
        // v3: sort_order 列（支持拖拽排序）
        migration_v3_sort_order,
        // v4: pinned 列（项目置顶）
        migration_v4_pinned,
        // v5: tasks.pinned 列（session 置顶）
        migration_v5_task_pinned,
        // v6: Agent 工作过程时间线
        migration_v6_agent_timeline_events,
        // v7: 放宽 kind CHECK，纳入 'message'（user/assistant 都走 timeline）
        migration_v7_timeline_message_kind,
    ];

    let current: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| format!("lilia-store: 读取 user_version 失败：{e}"))?;

    for (idx, migrate) in migrations.iter().enumerate() {
        let target = (idx as i64) + 1;
        if current >= target {
            continue;
        }
        migrate(conn)?;
        // PRAGMA user_version 不接受参数绑定，需用字面量拼接（安全：值由代码控制）。
        conn.execute_batch(&format!("PRAGMA user_version = {target};"))
            .map_err(|e| format!("lilia-store: 写 user_version 失败：{e}"))?;
    }
    Ok(())
}

fn migration_v1_task_todos(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS task_todos (
          id           TEXT PRIMARY KEY,
          task_id      TEXT NOT NULL,
          text         TEXT NOT NULL,
          done         INTEGER NOT NULL DEFAULT 0,
          "order"      INTEGER NOT NULL,
          source       TEXT NOT NULL CHECK (source IN ('user','agent')),
          created_at   INTEGER NOT NULL,
          updated_at   INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_task_todos_task_id_order
          ON task_todos(task_id, "order");
        "#,
    )
    .map_err(|e| format!("lilia-store v1: 建 task_todos 失败：{e}"))
}

fn migration_v2_projects_tasks(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS projects (
          id         TEXT PRIMARY KEY,
          name       TEXT NOT NULL,
          cwd        TEXT,
          created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tasks (
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
          FOREIGN KEY (project_id) REFERENCES projects(id)
        );

        CREATE INDEX IF NOT EXISTS idx_tasks_project_id
          ON tasks(project_id);
        CREATE INDEX IF NOT EXISTS idx_tasks_archived
          ON tasks(archived);

        CREATE TABLE IF NOT EXISTS task_dependencies (
          task_id       TEXT NOT NULL,
          depends_on_id TEXT NOT NULL,
          PRIMARY KEY (task_id, depends_on_id),
          FOREIGN KEY (task_id)       REFERENCES tasks(id) ON DELETE CASCADE,
          FOREIGN KEY (depends_on_id) REFERENCES tasks(id) ON DELETE CASCADE
        );
        "#,
    )
    .map_err(|e| format!("lilia-store v2: 建 projects/tasks 失败：{e}"))
}

fn migration_v3_sort_order(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        ALTER TABLE projects ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE tasks ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

        -- 已有数据按 created_at 初始化排序（ASC：越旧 sort_order 越小，越新越大）
        UPDATE projects SET sort_order = (
          SELECT COUNT(*) FROM projects p2 WHERE p2.created_at <= projects.created_at
        ) - 1;

        -- tasks 按 project_id 分组初始化，orphan (NULL) 单独排
        UPDATE tasks SET sort_order = (
          SELECT COUNT(*) FROM tasks t2
          WHERE t2.archived = 0
            AND (t2.project_id = tasks.project_id OR (t2.project_id IS NULL AND tasks.project_id IS NULL))
            AND t2.created_at >= tasks.created_at
        ) - 1;
        "#,
    )
    .map_err(|e| format!("lilia-store v3: 加 sort_order 列失败：{e}"))
}

fn migration_v4_pinned(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        ALTER TABLE projects ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
        "#,
    )
    .map_err(|e| format!("lilia-store v4: 加 pinned 列失败：{e}"))
}

fn migration_v5_task_pinned(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        ALTER TABLE tasks ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
        "#,
    )
    .map_err(|e| format!("lilia-store v5: 加 tasks.pinned 列失败：{e}"))
}

fn migration_v6_agent_timeline_events(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS agent_timeline_events (
          id          TEXT PRIMARY KEY,
          task_id     TEXT NOT NULL,
          turn_id     TEXT,
          backend     TEXT NOT NULL CHECK (backend IN ('claude','codex')),
          kind        TEXT NOT NULL CHECK (kind IN (
                        'reasoning','plan','todo_list','tool','command','subagent',
                        'file_change','mcp','web_search','error','turn'
                      )),
          status      TEXT NOT NULL,
          title       TEXT NOT NULL,
          summary     TEXT,
          payload     TEXT NOT NULL,
          created_at  INTEGER NOT NULL,
          updated_at  INTEGER NOT NULL,
          "order"     INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_agent_timeline_events_task_id_order
          ON agent_timeline_events(task_id, "order");
        "#,
    )
    .map_err(|e| format!("lilia-store v6: 建 agent_timeline_events 失败：{e}"))
}

/// SQLite 不支持 ALTER CHECK，rebuild 表把 kind 集合放宽到包含 'message'。
/// user 输入与 assistant 流式回复都按 message 走 timeline，schema 必须接得住。
fn migration_v7_timeline_message_kind(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE agent_timeline_events_v7 (
          id          TEXT PRIMARY KEY,
          task_id     TEXT NOT NULL,
          turn_id     TEXT,
          backend     TEXT NOT NULL CHECK (backend IN ('claude','codex')),
          kind        TEXT NOT NULL CHECK (kind IN (
                        'message','reasoning','plan','todo_list','tool','command',
                        'subagent','file_change','mcp','web_search','error','turn'
                      )),
          status      TEXT NOT NULL,
          title       TEXT NOT NULL,
          summary     TEXT,
          payload     TEXT NOT NULL,
          created_at  INTEGER NOT NULL,
          updated_at  INTEGER NOT NULL,
          "order"     INTEGER NOT NULL
        );

        INSERT INTO agent_timeline_events_v7
          (id, task_id, turn_id, backend, kind, status, title, summary,
           payload, created_at, updated_at, "order")
          SELECT id, task_id, turn_id, backend, kind, status, title, summary,
                 payload, created_at, updated_at, "order"
          FROM agent_timeline_events;

        DROP TABLE agent_timeline_events;
        ALTER TABLE agent_timeline_events_v7 RENAME TO agent_timeline_events;

        CREATE INDEX IF NOT EXISTS idx_agent_timeline_events_task_id_order
          ON agent_timeline_events(task_id, "order");
        "#,
    )
    .map_err(|e| format!("lilia-store v7: 放宽 timeline kind CHECK 失败：{e}"))
}
