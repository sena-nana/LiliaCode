/*!
 * Agent 工作过程时间线：把 runner 侧解析好的事件持久化到 SQLite。
 *
 * 本模块只提供契约与存取命令，不负责 NDJSON 映射，也不触碰 chat runner
 * 的 stdout 事件读取循环。
 *
 * 表结构故意只存「事实」字段：kind / status / title / summary / payload。display
 * （图标、中文动词、详情面板）是渲染时的视图缓存，由前端
 * `deriveTimelineDisplay()` 现算 —— 历史事件能跟着 display 规则的迭代自动更新。
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
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub order: Option<i64>,
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentTimelineEvent> {
    let payload_text: String = row.get(8)?;
    let payload = serde_json::from_str(&payload_text)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(8, Type::Text, Box::new(e)))?;
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
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        order: row.get(11)?,
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
    let existing_position: Option<(i64, i64)> = conn
        .query_row(
            r#"SELECT created_at, "order" FROM agent_timeline_events WHERE id = ?1"#,
            params![id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("agent_timeline_insert: 查询已有事件位置失败：{e}"))?;
    let created_at = input
        .created_at
        .or_else(|| existing_position.map(|(created_at, _)| created_at))
        .unwrap_or(now);
    let order = match input.order {
        Some(order) => order,
        None => existing_position
            .map(|(_, order)| order)
            .unwrap_or(next_order(conn, &input.task_id)?),
    };
    let updated_at = input.updated_at.unwrap_or(if existing_position.is_some() {
        now
    } else {
        created_at
    });
    let payload_text = serde_json::to_string(&input.payload)
        .map_err(|e| format!("agent_timeline_insert: payload 序列化失败：{e}"))?;

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
        created_at,
        updated_at,
        order,
    };

    conn.execute(
        r#"INSERT INTO agent_timeline_events
           (id, task_id, turn_id, backend, kind, status, title, summary, payload, created_at, updated_at, "order")
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
           ON CONFLICT(id) DO UPDATE SET
             task_id = excluded.task_id,
             turn_id = excluded.turn_id,
             backend = excluded.backend,
             kind = excluded.kind,
             status = excluded.status,
             title = excluded.title,
             summary = excluded.summary,
             payload = excluded.payload,
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
                      payload, created_at, updated_at, "order"
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

pub fn latest_session_id(
    conn: &Connection,
    task_id: &str,
    backend: &str,
) -> Result<Option<String>, String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT payload
               FROM agent_timeline_events
               WHERE task_id = ?1
                 AND backend = ?2
                 AND kind = 'turn'
                 AND payload LIKE '%"sessionId"%'
               ORDER BY updated_at DESC
               LIMIT 100"#,
        )
        .map_err(|e| format!("agent_timeline_latest_session_id: prepare 失败：{e}"))?;
    let rows = stmt
        .query_map(params![task_id, backend], |row| row.get::<_, String>(0))
        .map_err(|e| format!("agent_timeline_latest_session_id: query 失败：{e}"))?;

    for row in rows {
        let payload_text = row
            .map_err(|e| format!("agent_timeline_latest_session_id: row 失败：{e}"))?;
        let Ok(payload) = serde_json::from_str::<JsonValue>(&payload_text) else {
            continue;
        };
        let Some(session_id) = payload.get("sessionId").and_then(|v| v.as_str()) else {
            continue;
        };
        let trimmed = session_id.trim();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed.to_string()));
        }
    }
    Ok(None)
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
    fn insert_and_list_round_trip_unknown_kind() {
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
                created_at: Some(100),
                updated_at: Some(101),
                order: Some(1),
            },
        )
        .unwrap();

        assert_eq!(saved.kind, "extension_index");
        assert_eq!(saved.payload, json!({ "raw": true }));

        let listed = list(&conn, "task-1").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].kind, "extension_index");
        assert_eq!(listed[0].payload, json!({ "raw": true }));
        assert_eq!(listed[0].summary.as_deref(), Some("indexed"));
    }

    #[test]
    fn upsert_without_explicit_position_keeps_original_timeline_position() {
        let conn = Connection::open_in_memory().unwrap();
        create_timeline_schema(&conn);

        insert(
            &conn,
            AgentTimelineEventInput {
                id: Some("reasoning-1".to_string()),
                task_id: "task-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                backend: "claude".to_string(),
                kind: "reasoning".to_string(),
                status: "running".to_string(),
                title: "思考中".to_string(),
                summary: Some("first".to_string()),
                payload: json!({ "text": "first" }),
                created_at: Some(100),
                updated_at: Some(100),
                order: Some(1),
            },
        )
        .unwrap();
        insert(
            &conn,
            AgentTimelineEventInput {
                id: Some("command-1".to_string()),
                task_id: "task-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                backend: "claude".to_string(),
                kind: "command".to_string(),
                status: "success".to_string(),
                title: "yarn test".to_string(),
                summary: Some("command".to_string()),
                payload: json!({ "command": "yarn test" }),
                created_at: Some(200),
                updated_at: Some(200),
                order: Some(2),
            },
        )
        .unwrap();

        let updated = insert(
            &conn,
            AgentTimelineEventInput {
                id: Some("reasoning-1".to_string()),
                task_id: "task-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                backend: "claude".to_string(),
                kind: "reasoning".to_string(),
                status: "success".to_string(),
                title: "已思考".to_string(),
                summary: Some("first completed".to_string()),
                payload: json!({ "text": "first completed" }),
                created_at: None,
                updated_at: None,
                order: None,
            },
        )
        .unwrap();

        assert_eq!(updated.created_at, 100);
        assert_eq!(updated.order, 1);

        let listed = list(&conn, "task-1").unwrap();
        assert_eq!(
            listed.iter().map(|event| event.id.as_str()).collect::<Vec<_>>(),
            vec!["reasoning-1", "command-1"],
        );
        assert_eq!(listed[0].summary.as_deref(), Some("first completed"));
    }

    #[test]
    fn latest_session_id_reads_recent_turn_payload() {
        let conn = Connection::open_in_memory().unwrap();
        create_timeline_schema(&conn);

        insert(
            &conn,
            AgentTimelineEventInput {
                id: Some("turn-old".to_string()),
                task_id: "task-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                backend: "claude".to_string(),
                kind: "turn".to_string(),
                status: "requesting".to_string(),
                title: "Claude status".to_string(),
                summary: None,
                payload: json!({ "sessionId": "old-session" }),
                created_at: Some(100),
                updated_at: Some(100),
                order: Some(1),
            },
        )
        .unwrap();
        insert(
            &conn,
            AgentTimelineEventInput {
                id: Some("turn-new".to_string()),
                task_id: "task-1".to_string(),
                turn_id: Some("turn-2".to_string()),
                backend: "claude".to_string(),
                kind: "turn".to_string(),
                status: "requesting".to_string(),
                title: "Claude status".to_string(),
                summary: None,
                payload: json!({ "sessionId": "new-session" }),
                created_at: Some(200),
                updated_at: Some(200),
                order: Some(2),
            },
        )
        .unwrap();

        assert_eq!(
            latest_session_id(&conn, "task-1", "claude").unwrap(),
            Some("new-session".to_string())
        );
    }
}
