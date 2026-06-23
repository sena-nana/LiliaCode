use rusqlite::Connection;

use crate::agent_timeline::{self, AgentTimelineEvent, AgentTimelineEventInput};

pub(crate) fn preview_events_from_inputs(
    inputs: Vec<AgentTimelineEventInput>,
) -> Result<Vec<AgentTimelineEvent>, String> {
    let conn = Connection::open_in_memory()
        .map_err(|e| format!("创建 preview timeline 内存库失败：{e}"))?;
    agent_timeline::create_timeline_schema(&conn)
        .map_err(|e| format!("创建 preview timeline schema 失败：{e}"))?;
    for input in inputs {
        agent_timeline::insert(&conn, input)?;
    }
    agent_timeline::list(&conn, "preview")
}
