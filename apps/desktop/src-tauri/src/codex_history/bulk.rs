use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::agent_timeline::{AgentTimelineEvent, AgentTimelineEventInput};
use crate::chat::contract;
use crate::chat::timeline_sink::persist_and_emit_input;
use crate::store::LiliaStore;
use crate::util::now_millis;

use super::types::AgentTimelineBatchPayload;

fn history_id_prefix(event: &AgentTimelineEventInput) -> &'static str {
    match event.backend.as_str() {
        "claude" => "claude-history",
        _ => "codex-history",
    }
}

fn stable_history_event_id(task_id: &str, event: &AgentTimelineEventInput) -> Option<String> {
    let payload = event.payload.as_object()?;
    let thread_id = payload.get("threadId")?.as_str()?;
    let turn_id = payload
        .get("turnId")
        .and_then(|value| value.as_str())
        .or(event.turn_id.as_deref())?;
    let item_id = payload.get("itemId")?.as_str()?;
    let prefix = history_id_prefix(event);
    Some(format!(
        "{task_id}:{turn_id}:{prefix}:{thread_id}:{turn_id}:{item_id}"
    ))
}

fn persist_history_events(
    app: &AppHandle,
    task_id: &str,
    events: Vec<AgentTimelineEventInput>,
) -> usize {
    let mut count = 0;
    for mut event in events {
        event.task_id = task_id.to_string();
        if let Some(id) = stable_history_event_id(task_id, &event) {
            event.id = Some(id);
        }
        persist_and_emit_input(app, event);
        count += 1;
    }
    count
}

#[derive(Debug, Clone)]
struct ExistingTimelinePosition {
    created_at: i64,
    turn_seq: i64,
    intra_turn_order: i64,
}

fn query_existing_position(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<Option<ExistingTimelinePosition>, String> {
    conn.query_row(
        r#"SELECT created_at, turn_seq, intra_turn_order
           FROM agent_timeline_events
           WHERE id = ?1"#,
        params![id],
        |row| {
            Ok(ExistingTimelinePosition {
                created_at: row.get(0)?,
                turn_seq: row.get(1)?,
                intra_turn_order: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(|e| format!("Codex history bulk: 查询已有事件失败：{e}"))
}

fn query_turn_seq(
    conn: &rusqlite::Connection,
    task_id: &str,
    turn_id: Option<&str>,
) -> Result<Option<i64>, String> {
    conn.query_row(
        r#"SELECT turn_seq FROM agent_timeline_events
           WHERE task_id = ?1 AND turn_id IS ?2
           LIMIT 1"#,
        params![task_id, turn_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| format!("Codex history bulk: 查询 turn_seq 失败：{e}"))
}

fn query_next_intra_turn_order(
    conn: &rusqlite::Connection,
    task_id: &str,
    turn_seq: i64,
) -> Result<i64, String> {
    let max: Option<i64> = conn
        .query_row(
            r#"SELECT MAX(intra_turn_order) FROM agent_timeline_events
               WHERE task_id = ?1 AND turn_seq = ?2"#,
            params![task_id, turn_seq],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Codex history bulk: 查询 intra_turn_order 失败：{e}"))?
        .flatten();
    Ok(max.map_or(0, |value| value + 1))
}

fn query_history_insert_at(
    conn: &rusqlite::Connection,
    task_id: &str,
    created_at: i64,
) -> Result<i64, String> {
    let insert_at: Option<i64> = conn
        .query_row(
            r#"SELECT turn_seq
               FROM (
                 SELECT turn_seq, MIN(created_at) AS first_created_at
                 FROM agent_timeline_events
                 WHERE task_id = ?1
                 GROUP BY turn_seq
               )
               WHERE first_created_at > ?2
               ORDER BY first_created_at ASC, turn_seq ASC
               LIMIT 1"#,
            params![task_id, created_at],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Codex history bulk: 查询插入位置失败：{e}"))?;
    if let Some(seq) = insert_at {
        return Ok(seq);
    }
    let max: Option<i64> = conn
        .query_row(
            r#"SELECT MAX(turn_seq) FROM agent_timeline_events WHERE task_id = ?1"#,
            params![task_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Codex history bulk: 查询 max(turn_seq) 失败：{e}"))?
        .flatten();
    Ok(max.map_or(0, |value| value + 1))
}

fn turn_key(turn_id: Option<&str>) -> String {
    turn_id.unwrap_or("\u{0}").to_string()
}

fn bulk_history_events(
    conn: &rusqlite::Connection,
    task_id: &str,
    events: Vec<AgentTimelineEventInput>,
) -> Result<Vec<AgentTimelineEvent>, String> {
    let now = now_millis() as i64;
    let mut rows = Vec::new();
    for mut input in events {
        input.task_id = task_id.to_string();
        if let Some(id) = stable_history_event_id(task_id, &input) {
            input.id = Some(id);
        }
        let id = input
            .id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let existing = query_existing_position(conn, &id)?;
        rows.push((id, input, existing));
    }
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let mut turn_seq_by_key: HashMap<String, i64> = HashMap::new();
    let mut new_turns: Vec<(String, Option<String>, i64, usize)> = Vec::new();
    let mut new_turn_index_by_key: HashMap<String, usize> = HashMap::new();
    for (index, (_id, input, existing)) in rows.iter().enumerate() {
        if let Some(position) = existing {
            turn_seq_by_key.insert(turn_key(input.turn_id.as_deref()), position.turn_seq);
            continue;
        }
        let key = turn_key(input.turn_id.as_deref());
        if turn_seq_by_key.contains_key(&key) || new_turn_index_by_key.contains_key(&key) {
            continue;
        }
        if let Some(seq) = query_turn_seq(conn, task_id, input.turn_id.as_deref())? {
            turn_seq_by_key.insert(key, seq);
            continue;
        }
        let created_at = input.created_at.unwrap_or(now);
        new_turn_index_by_key.insert(key.clone(), new_turns.len());
        new_turns.push((key, input.turn_id.clone(), created_at, index));
    }

    new_turns.sort_by(|a, b| a.2.cmp(&b.2).then(a.3.cmp(&b.3)));
    if let Some((_, _, first_created_at, _)) = new_turns.first() {
        let insert_at = query_history_insert_at(conn, task_id, *first_created_at)?;
        conn.execute(
            r#"UPDATE agent_timeline_events
               SET turn_seq = turn_seq + ?2
               WHERE task_id = ?1 AND turn_seq >= ?3"#,
            params![task_id, new_turns.len() as i64, insert_at],
        )
        .map_err(|e| format!("Codex history bulk: 后移已有 turn_seq 失败：{e}"))?;
        let shift_by = new_turns.len() as i64;
        for (_id, _input, existing) in rows.iter_mut() {
            if let Some(position) = existing
                .as_mut()
                .filter(|position| position.turn_seq >= insert_at)
            {
                position.turn_seq += shift_by;
            }
        }
        for seq in turn_seq_by_key.values_mut() {
            if *seq >= insert_at {
                *seq += shift_by;
            }
        }
        for (offset, (key, _turn_id, _created_at, _index)) in new_turns.iter().enumerate() {
            turn_seq_by_key.insert(key.clone(), insert_at + offset as i64);
        }
    }

    let mut next_intra_by_seq: HashMap<i64, i64> = HashMap::new();
    let mut saved = Vec::new();
    for (id, input, existing) in rows {
        let key = turn_key(input.turn_id.as_deref());
        let turn_seq = existing
            .as_ref()
            .map(|position| position.turn_seq)
            .or_else(|| turn_seq_by_key.get(&key).copied())
            .ok_or_else(|| "Codex history bulk: 缺少 turn_seq".to_string())?;
        let intra_turn_order = if let Some(position) = existing.as_ref() {
            position.intra_turn_order
        } else {
            let next = match next_intra_by_seq.get(&turn_seq).copied() {
                Some(value) => value,
                None => query_next_intra_turn_order(conn, task_id, turn_seq)?,
            };
            next_intra_by_seq.insert(turn_seq, next + 1);
            next
        };
        let created_at = input
            .created_at
            .or_else(|| existing.as_ref().map(|position| position.created_at))
            .unwrap_or(now);
        let updated_at =
            input
                .updated_at
                .unwrap_or(if existing.is_some() { now } else { created_at });
        let payload_text = serde_json::to_string(&input.payload)
            .map_err(|e| format!("Codex history bulk: payload 序列化失败：{e}"))?;
        let event = AgentTimelineEvent {
            id,
            task_id: task_id.to_string(),
            turn_id: input.turn_id,
            backend: input.backend,
            kind: input.kind,
            status: input.status,
            title: input.title,
            summary: input.summary,
            payload: input.payload,
            created_at,
            updated_at,
            turn_seq,
            intra_turn_order,
        };
        conn.execute(
            r#"INSERT INTO agent_timeline_events
               (id, task_id, turn_id, backend, kind, status, title, summary, payload,
                created_at, updated_at, turn_seq, intra_turn_order)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
               ON CONFLICT(id) DO UPDATE SET
                 task_id          = excluded.task_id,
                 turn_id          = excluded.turn_id,
                 backend          = excluded.backend,
                 kind             = excluded.kind,
                 status           = excluded.status,
                 title            = excluded.title,
                 summary          = excluded.summary,
                 payload          = excluded.payload,
                 created_at       = excluded.created_at,
                 updated_at       = excluded.updated_at,
                 turn_seq         = excluded.turn_seq,
                 intra_turn_order = excluded.intra_turn_order"#,
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
                event.turn_seq,
                event.intra_turn_order,
            ],
        )
        .map_err(|e| format!("Codex history bulk: 写入失败：{e}"))?;
        saved.push(event);
    }
    Ok(saved)
}

const HISTORY_EMIT_BATCH_SIZE: usize = 80;

fn emit_history_event_batches(app: &AppHandle, task_id: &str, events: Vec<AgentTimelineEvent>) {
    for chunk in events.chunks(HISTORY_EMIT_BATCH_SIZE) {
        let _ = app.emit(
            contract::agent_timeline_batch_event_name(),
            AgentTimelineBatchPayload {
                task_id: task_id.to_string(),
                events: chunk.to_vec(),
            },
        );
    }
}

pub(crate) fn persist_history_events_batch(
    app: &AppHandle,
    task_id: &str,
    events: Vec<AgentTimelineEventInput>,
) -> usize {
    let store = app.state::<LiliaStore>();
    let Ok(conn) = store.conn() else {
        return persist_history_events(app, task_id, events);
    };
    if let Err(err) = conn.execute_batch("BEGIN IMMEDIATE;") {
        eprintln!("[codex-history] background history transaction failed: {err}");
        return persist_history_events(app, task_id, events);
    }
    let saved_events = match bulk_history_events(&conn, task_id, events) {
        Ok(events) => events,
        Err(err) => {
            eprintln!("[codex-history] background history persist failed: {err}");
            let _ = conn.execute_batch("ROLLBACK;");
            return 0;
        }
    };
    if let Err(err) = conn.execute_batch("COMMIT;") {
        eprintln!("[codex-history] background history commit failed: {err}");
        let _ = conn.execute_batch("ROLLBACK;");
        return 0;
    }
    let count = saved_events.len();
    emit_history_event_batches(app, task_id, saved_events);
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_timeline;

    fn session_anchor_input(task_id: &str, thread_id: &str, now: i64) -> AgentTimelineEventInput {
        AgentTimelineEventInput {
            id: Some(format!("{task_id}:codex-thread-attach:{thread_id}")),
            task_id: task_id.to_string(),
            turn_id: Some(format!("codex-thread-attach:{thread_id}")),
            backend: "codex".to_string(),
            kind: "turn".to_string(),
            status: "success".to_string(),
            title: "Codex thread attached".to_string(),
            summary: Some("已接入 Codex thread".to_string()),
            payload: serde_json::json!({
                "backend": "codex",
                "sessionId": thread_id,
                "subkind": "thread_attach",
            }),
            created_at: Some(now),
            updated_at: Some(now),
        }
    }

    fn history_input(turn_id: &str, item_id: &str, created_at: i64) -> AgentTimelineEventInput {
        AgentTimelineEventInput {
            id: None,
            task_id: "pending".to_string(),
            turn_id: Some(turn_id.to_string()),
            backend: "codex".to_string(),
            kind: "message".to_string(),
            status: "success".to_string(),
            title: "Assistant".to_string(),
            summary: Some(item_id.to_string()),
            payload: serde_json::json!({
                "history": true,
                "threadId": "thread-1",
                "turnId": turn_id,
                "itemId": item_id,
            }),
            created_at: Some(created_at),
            updated_at: Some(created_at),
        }
    }

    #[test]
    fn stable_history_event_id_uses_thread_turn_and_item() {
        let input = history_input("turn-1", "msg-1", 1);

        assert_eq!(
            stable_history_event_id("task-1", &input).as_deref(),
            Some("task-1:turn-1:codex-history:thread-1:turn-1:msg-1")
        );
    }

    #[test]
    fn bulk_history_events_insert_before_anchor_with_single_shift() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        agent_timeline::create_timeline_schema(&conn).unwrap();
        agent_timeline::insert(&conn, session_anchor_input("task-1", "thread-1", 10_000)).unwrap();

        let saved = bulk_history_events(
            &conn,
            "task-1",
            vec![
                history_input("turn-old-1", "msg-1", 1_000),
                history_input("turn-old-2", "msg-2", 2_000),
            ],
        )
        .unwrap();

        assert_eq!(saved.len(), 2);
        let listed: Vec<_> = agent_timeline::list(&conn, "task-1")
            .unwrap()
            .into_iter()
            .map(|event| {
                (
                    event.turn_id.unwrap_or_default(),
                    event.turn_seq,
                    event.intra_turn_order,
                )
            })
            .collect();
        assert_eq!(
            listed,
            vec![
                ("turn-old-1".to_string(), 0, 0),
                ("turn-old-2".to_string(), 1, 0),
                ("codex-thread-attach:thread-1".to_string(), 2, 0),
            ]
        );
    }

    #[test]
    fn bulk_history_events_repeated_sync_keeps_stable_positions() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        agent_timeline::create_timeline_schema(&conn).unwrap();
        agent_timeline::insert(&conn, session_anchor_input("task-1", "thread-1", 10_000)).unwrap();
        let events = vec![
            history_input("turn-old-1", "msg-1", 1_000),
            history_input("turn-old-2", "msg-2", 2_000),
        ];

        bulk_history_events(&conn, "task-1", events.clone()).unwrap();
        let first: Vec<_> = agent_timeline::list(&conn, "task-1")
            .unwrap()
            .into_iter()
            .map(|event| (event.id, event.turn_seq, event.intra_turn_order))
            .collect();
        bulk_history_events(&conn, "task-1", events).unwrap();
        let second: Vec<_> = agent_timeline::list(&conn, "task-1")
            .unwrap()
            .into_iter()
            .map(|event| (event.id, event.turn_seq, event.intra_turn_order))
            .collect();

        assert_eq!(second, first);
        assert_eq!(second.len(), 3);
    }
}
