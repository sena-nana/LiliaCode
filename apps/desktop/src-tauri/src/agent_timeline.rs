/*!
 * Agent 工作过程时间线：把 runner 侧解析好的事件持久化到 SQLite。
 *
 * 本模块只提供契约与存取命令，不负责 NDJSON 映射，也不触碰 chat runner
 * 的 stdout 事件读取循环。
 */

use rusqlite::{params, types::Type, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::State;
use uuid::Uuid;

use crate::store::LiliaStore;
use crate::util::now_millis;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTimelineEvent {
    pub id: String,
    pub task_id: String,
    pub turn_id: Option<String>,
    /// "claude" | "codex"
    pub backend: String,
    /// "reasoning" | "plan" | "todo_list" | "tool" | ...
    pub kind: String,
    pub status: String,
    pub title: String,
    pub summary: Option<String>,
    pub payload: JsonValue,
    pub display: JsonValue,
    pub created_at: i64,
    pub updated_at: i64,
    pub order: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTimelineEventInput {
    pub id: Option<String>,
    pub task_id: String,
    pub turn_id: Option<String>,
    pub backend: String,
    pub kind: String,
    pub status: String,
    pub title: String,
    pub summary: Option<String>,
    #[serde(default)]
    pub payload: JsonValue,
    pub display: JsonValue,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub order: Option<i64>,
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentTimelineEvent> {
    let payload_text: String = row.get(8)?;
    let display_text: String = row.get(9)?;
    let payload = serde_json::from_str(&payload_text)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(8, Type::Text, Box::new(e)))?;
    let display = serde_json::from_str(&display_text)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(9, Type::Text, Box::new(e)))?;
    Ok(AgentTimelineEvent {
        id: row.get(0)?,
        task_id: row.get(1)?,
        turn_id: row.get(2)?,
        backend: row.get(3)?,
        kind: row.get(4)?,
        status: row.get(5)?,
        title: row.get(6)?,
        summary: row.get(7)?,
        payload,
        display,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        order: row.get(12)?,
    })
}

fn next_order(conn: &Connection, task_id: &str) -> Result<i64, String> {
    let max: Option<i64> = conn
        .query_row(
            r#"SELECT MAX("order") FROM agent_timeline_events WHERE task_id = ?1"#,
            params![task_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("agent_timeline: 查询 max(order) 失败：{e}"))?
        .flatten();
    Ok(max.unwrap_or(-1) + 1)
}

pub fn insert(
    conn: &Connection,
    input: AgentTimelineEventInput,
) -> Result<AgentTimelineEvent, String> {
    let now = now_millis();
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let order = match input.order {
        Some(order) => order,
        None => next_order(conn, &input.task_id)?,
    };
    let created_at = input.created_at.unwrap_or(now);
    let updated_at = input.updated_at.unwrap_or(created_at);
    let payload_text = serde_json::to_string(&input.payload)
        .map_err(|e| format!("agent_timeline_insert: payload 序列化失败：{e}"))?;
    let display_text = serde_json::to_string(&input.display)
        .map_err(|e| format!("agent_timeline_insert: display 序列化失败：{e}"))?;

    let event = AgentTimelineEvent {
        id,
        task_id: input.task_id,
        turn_id: input.turn_id,
        backend: input.backend,
        kind: input.kind,
        status: input.status,
        title: input.title,
        summary: input.summary,
        payload: input.payload,
        display: input.display,
        created_at,
        updated_at,
        order,
    };

    conn.execute(
        r#"INSERT INTO agent_timeline_events
           (id, task_id, turn_id, backend, kind, status, title, summary, payload, display, created_at, updated_at, "order")
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
           ON CONFLICT(id) DO UPDATE SET
             task_id = excluded.task_id,
             turn_id = excluded.turn_id,
             backend = excluded.backend,
             kind = excluded.kind,
             status = excluded.status,
             title = excluded.title,
             summary = excluded.summary,
             payload = excluded.payload,
             display = excluded.display,
             created_at = excluded.created_at,
             updated_at = excluded.updated_at,
             "order" = excluded."order""#,
        params![
            event.id,
            event.task_id,
            event.turn_id,
            event.backend,
            event.kind,
            event.status,
            event.title,
            event.summary,
            payload_text,
            display_text,
            event.created_at,
            event.updated_at,
            event.order,
        ],
    )
    .map_err(|e| format!("agent_timeline_insert: 写入失败：{e}"))?;

    Ok(event)
}

pub fn list(conn: &Connection, task_id: &str) -> Result<Vec<AgentTimelineEvent>, String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT id, task_id, turn_id, backend, kind, status, title, summary,
                      payload, display, created_at, updated_at, "order"
               FROM agent_timeline_events
               WHERE task_id = ?1
               ORDER BY "order" ASC, created_at ASC"#,
        )
        .map_err(|e| format!("agent_timeline_list: prepare 失败：{e}"))?;
    let rows = stmt
        .query_map(params![task_id], row_to_event)
        .map_err(|e| format!("agent_timeline_list: query 失败：{e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("agent_timeline_list: 行解析失败：{e}"))?);
    }
    Ok(out)
}

pub fn clear(conn: &Connection, task_id: &str) -> Result<usize, String> {
    conn.execute(
        "DELETE FROM agent_timeline_events WHERE task_id = ?1",
        params![task_id],
    )
    .map_err(|e| format!("agent_timeline_clear_task: 删除失败：{e}"))
}

#[tauri::command]
pub fn agent_timeline_list(
    task_id: String,
    store: State<'_, LiliaStore>,
) -> Result<Vec<AgentTimelineEvent>, String> {
    let conn = store.conn()?;
    list(&conn, &task_id)
}

#[tauri::command]
pub fn agent_timeline_clear_task(
    task_id: String,
    store: State<'_, LiliaStore>,
) -> Result<usize, String> {
    let conn = store.conn()?;
    clear(&conn, &task_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_timeline_schema(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE agent_timeline_events (
              id          TEXT PRIMARY KEY,
              task_id     TEXT NOT NULL,
              turn_id     TEXT,
              backend     TEXT NOT NULL CHECK (backend IN ('claude','codex')),
              kind        TEXT NOT NULL,
              status      TEXT NOT NULL,
              title       TEXT NOT NULL,
              summary     TEXT,
              payload     TEXT NOT NULL,
              display     TEXT NOT NULL,
              created_at  INTEGER NOT NULL,
              updated_at  INTEGER NOT NULL,
              "order"     INTEGER NOT NULL
            );
            CREATE INDEX idx_agent_timeline_events_task_id_order
              ON agent_timeline_events(task_id, "order");
            "#,
        )
        .unwrap();
    }

    #[test]
    fn insert_and_list_preserves_display_for_unknown_kind() {
        let conn = Connection::open_in_memory().unwrap();
        create_timeline_schema(&conn);

        let saved = insert(
            &conn,
            AgentTimelineEventInput {
                id: Some("event-1".to_string()),
                task_id: "task-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                backend: "claude".to_string(),
                kind: "extension_index".to_string(),
                status: "success".to_string(),
                title: "Index".to_string(),
                summary: Some("indexed".to_string()),
                payload: json!({ "raw": true }),
                display: json!({
                    "icon": "tool",
                    "action": "同步",
                    "object": "索引",
                    "group": { "key": "extension:index", "bucket": "index", "unit": "次索引", "count": 2 }
                }),
                created_at: Some(100),
                updated_at: Some(101),
                order: Some(1),
            },
        )
        .unwrap();

        assert_eq!(saved.kind, "extension_index");
        assert_eq!(
            saved.display,
            json!({
                "icon": "tool",
                "action": "同步",
                "object": "索引",
                "group": { "key": "extension:index", "bucket": "index", "unit": "次索引", "count": 2 }
            })
        );

        let listed = list(&conn, "task-1").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].kind, "extension_index");
        assert_eq!(listed[0].display, saved.display);
    }

}
