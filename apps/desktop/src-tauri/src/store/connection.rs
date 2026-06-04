use std::path::Path;

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;

use super::home::ensure_layout;
use super::schema::{ensure_current_schema, reset_orphaned_queued_guides};

pub struct LiliaStore {
    pool: Pool<SqliteConnectionManager>,
}

impl LiliaStore {
    pub fn new(home: &Path) -> Result<Self, String> {
        ensure_layout(home).map_err(|e| format!("lilia-store: 准备目录失败：{e}"))?;
        let db_path = home.join("db").join("lilia.db");

        let manager = SqliteConnectionManager::file(&db_path).with_init(|conn| {
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
