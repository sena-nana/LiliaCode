use rusqlite::Connection;

use crate::agent_timeline::{self, AgentTimelineEvent, AgentTimelineEventInput};

pub(super) fn preview_events_from_inputs(
    inputs: Vec<AgentTimelineEventInput>,
) -> Result<Vec<AgentTimelineEvent>, String> {
    let conn = Connection::open_in_memory()
        .map_err(|e| format!("创建 preview timeline 内存库失败：{e}"))?;
    conn.execute_batch(
        r#"
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
    .map_err(|e| format!("创建 preview timeline schema 失败：{e}"))?;
    for input in inputs {
        agent_timeline::insert(&conn, input)?;
    }
    agent_timeline::list(&conn, "preview")
}
