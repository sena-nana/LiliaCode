/*!
 * Todo 命令组：任务内 checklist，定位是「AI 思考过程可视化」。
 *
 * - 数据全部走 [`crate::store::LiliaStore`]，schema 由当前开发库基线创建。
 * - 自动通道：provider `todo_list` 运行时事件 → [`apply_agent_event_impl`]；兼容
 *   Claude `tool_use { name: "TodoWrite" }`。落库时按 `text` 匹配现有 source="agent"
 *   的行做 upsert，并删掉本次没出现的 agent 行；`source="user"` 的行不受影响。
 */

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::State;
use uuid::Uuid;

use crate::store::LiliaStore;
use crate::util::now_millis;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskTodo {
    pub id: String,
    pub task_id: String,
    pub text: String,
    pub done: bool,
    pub order: i64,
    /// "user" | "agent"
    pub source: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Provider todo 事件携带的单条 todo；兼容 Claude `content/status` 与 Codex `text/completed`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTodoItem {
    #[serde(alias = "text", alias = "title", alias = "description")]
    pub content: String,
    /// "pending" | "in_progress" | "completed"
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub completed: Option<bool>,
    #[serde(default)]
    pub done: Option<bool>,
}

impl AgentTodoItem {
    fn is_done(&self) -> bool {
        self.completed.unwrap_or(false)
            || self.done.unwrap_or(false)
            || self.status.eq_ignore_ascii_case("completed")
    }
}

pub(crate) fn parse_agent_todo_items(values: &[JsonValue]) -> Vec<AgentTodoItem> {
    values
        .iter()
        .filter_map(|value| {
            if let Some(text) = value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            {
                return Some(AgentTodoItem {
                    content: text.to_string(),
                    status: "pending".to_string(),
                    completed: None,
                    done: None,
                });
            }
            let mut item = serde_json::from_value::<AgentTodoItem>(value.clone()).ok()?;
            item.content = item.content.trim().to_string();
            if item.content.is_empty() {
                return None;
            }
            Some(item)
        })
        .collect()
}

fn row_to_todo(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskTodo> {
    Ok(TaskTodo {
        id: row.get(0)?,
        task_id: row.get(1)?,
        text: row.get(2)?,
        done: row.get::<_, i64>(3)? != 0,
        order: row.get(4)?,
        source: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn select_by_task(conn: &Connection, task_id: &str) -> Result<Vec<TaskTodo>, String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT id, task_id, text, done, "order", source, created_at, updated_at
               FROM task_todos WHERE task_id = ?1 ORDER BY "order" ASC, created_at ASC"#,
        )
        .map_err(|e| format!("todo_list: prepare 失败：{e}"))?;
    let rows = stmt
        .query_map(params![task_id], row_to_todo)
        .map_err(|e| format!("todo_list: query 失败：{e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("todo_list: 行解析失败：{e}"))?);
    }
    Ok(out)
}

fn next_order(conn: &Connection, task_id: &str) -> Result<i64, String> {
    let max: Option<i64> = conn
        .query_row(
            r#"SELECT MAX("order") FROM task_todos WHERE task_id = ?1"#,
            params![task_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("todo: 查询 max(order) 失败：{e}"))?
        .flatten();
    Ok(max.unwrap_or(-1) + 1)
}

#[tauri::command]
pub fn todo_list(task_id: String, store: State<'_, LiliaStore>) -> Result<Vec<TaskTodo>, String> {
    let conn = store.conn()?;
    select_by_task(&conn, &task_id)
}

#[tauri::command]
pub fn todo_create(
    task_id: String,
    text: String,
    store: State<'_, LiliaStore>,
) -> Result<TaskTodo, String> {
    let conn = store.conn()?;
    let id = Uuid::new_v4().to_string();
    let order = next_order(&conn, &task_id)?;
    let now = now_millis();
    let todo = TaskTodo {
        id: id.clone(),
        task_id: task_id.clone(),
        text,
        done: false,
        order,
        source: "user".to_string(),
        created_at: now,
        updated_at: now,
    };
    conn.execute(
        r#"INSERT INTO task_todos (id, task_id, text, done, "order", source, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        params![
            todo.id,
            todo.task_id,
            todo.text,
            if todo.done { 1 } else { 0 },
            todo.order,
            todo.source,
            todo.created_at,
            todo.updated_at,
        ],
    )
    .map_err(|e| format!("todo_create: insert 失败：{e}"))?;
    Ok(todo)
}

#[tauri::command]
pub fn todo_update(
    id: String,
    text: Option<String>,
    done: Option<bool>,
    order: Option<i64>,
    store: State<'_, LiliaStore>,
) -> Result<(), String> {
    if text.is_none() && done.is_none() && order.is_none() {
        return Ok(());
    }
    let conn = store.conn()?;
    let now = now_millis();
    // 分别更新需要变的列，避免拼 SQL；3 个字段都独立 UPDATE 一次开销可忽略。
    if let Some(t) = text {
        conn.execute(
            "UPDATE task_todos SET text = ?1, updated_at = ?2 WHERE id = ?3",
            params![t, now, id],
        )
        .map_err(|e| format!("todo_update(text): {e}"))?;
    }
    if let Some(d) = done {
        conn.execute(
            "UPDATE task_todos SET done = ?1, updated_at = ?2 WHERE id = ?3",
            params![if d { 1 } else { 0 }, now, id],
        )
        .map_err(|e| format!("todo_update(done): {e}"))?;
    }
    if let Some(o) = order {
        conn.execute(
            r#"UPDATE task_todos SET "order" = ?1, updated_at = ?2 WHERE id = ?3"#,
            params![o, now, id],
        )
        .map_err(|e| format!("todo_update(order): {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn todo_delete(id: String, store: State<'_, LiliaStore>) -> Result<(), String> {
    let conn = store.conn()?;
    conn.execute("DELETE FROM task_todos WHERE id = ?1", params![id])
        .map_err(|e| format!("todo_delete: {e}"))?;
    Ok(())
}

/// 把标准化 agent todo 列表落库。复用规则：
/// - 按 `text` 等值匹配现有 `source="agent"` 行；命中则保留 id、更新 done/order/updatedAt
/// - 未命中的新条目以 `source="agent"` 插入
/// - 本次没出现的 agent 行删除（保持 agent 列表 = 最新 SDK 状态）
/// - `source="user"` 的行原样保留，order 维持
pub fn apply_agent_event_impl(
    conn: &Connection,
    task_id: &str,
    todos: &[AgentTodoItem],
) -> Result<Vec<TaskTodo>, String> {
    let now = now_millis();

    // 现有 agent 行：text -> (id, order, created_at)
    let mut existing: std::collections::HashMap<String, (String, i64, i64)> =
        std::collections::HashMap::new();
    {
        let mut stmt = conn
            .prepare(
                r#"SELECT id, text, "order", created_at FROM task_todos
                   WHERE task_id = ?1 AND source = 'agent'"#,
            )
            .map_err(|e| format!("apply_agent_event: prepare 失败：{e}"))?;
        let rows = stmt
            .query_map(params![task_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|e| format!("apply_agent_event: query 失败：{e}"))?;
        for r in rows {
            let (id, text, order, created_at) =
                r.map_err(|e| format!("apply_agent_event: row 失败：{e}"))?;
            existing.insert(text, (id, order, created_at));
        }
    }

    // 计算 user 行已经占走的 max(order)，给本次 agent 顺序作为起点的兜底。
    // 仍然给 agent 一个独立 order 段（接 user 末尾递增），避免重排 user 行。
    let user_max: i64 = conn
        .query_row(
            r#"SELECT COALESCE(MAX("order"), -1) FROM task_todos
               WHERE task_id = ?1 AND source = 'user'"#,
            params![task_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("apply_agent_event: 查 user_max 失败：{e}"))?;

    let mut seen_texts: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (idx, item) in todos.iter().enumerate() {
        let text = item.content.trim().to_string();
        if text.is_empty() {
            continue;
        }
        let done = item.is_done();
        let order = user_max + 1 + (idx as i64);
        seen_texts.insert(text.clone());

        if let Some((id, _old_order, _created)) = existing.get(&text) {
            conn.execute(
                r#"UPDATE task_todos
                   SET done = ?1, "order" = ?2, updated_at = ?3
                   WHERE id = ?4"#,
                params![if done { 1 } else { 0 }, order, now, id],
            )
            .map_err(|e| format!("apply_agent_event: update 失败：{e}"))?;
        } else {
            let id = Uuid::new_v4().to_string();
            conn.execute(
                r#"INSERT INTO task_todos
                   (id, task_id, text, done, "order", source, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, 'agent', ?6, ?7)"#,
                params![id, task_id, text, if done { 1 } else { 0 }, order, now, now],
            )
            .map_err(|e| format!("apply_agent_event: insert 失败：{e}"))?;
        }
    }

    // 删除本次没出现的 agent 行
    for (text, (id, _, _)) in &existing {
        if !seen_texts.contains(text) {
            conn.execute("DELETE FROM task_todos WHERE id = ?1", params![id])
                .map_err(|e| format!("apply_agent_event: delete 失败：{e}"))?;
        }
    }

    select_by_task(conn, task_id)
}

#[tauri::command]
pub fn todo_apply_agent_event(
    task_id: String,
    todos: Vec<AgentTodoItem>,
    store: State<'_, LiliaStore>,
) -> Result<Vec<TaskTodo>, String> {
    let conn = store.conn()?;
    apply_agent_event_impl(&conn, &task_id, &todos)
}
