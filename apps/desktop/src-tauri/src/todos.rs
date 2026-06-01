/*!
 * Todo 命令组：任务内 checklist，定位是「AI 思考过程可视化」。
 *
 * - 数据全部走 [`crate::store::LiliaStore`]，schema 由当前开发库基线创建。
 * - 自动通道：provider `todo_list` 运行时事件 → [`apply_agent_event_impl`]；兼容
 *   Claude `tool_use { name: "TodoWrite" }`。落库时按 `text` 匹配现有 source="agent"
 *   的行做 upsert，并删掉本次没出现的 agent 行；`source="lilia"` 的引导不受影响。
 */

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Emitter, Runtime, State};
use uuid::Uuid;

use crate::store::LiliaStore;
use crate::util::now_millis;

const PRIORITY_HIGH: &str = "high";
const PRIORITY_NORMAL: &str = "normal";
const PRIORITY_LOW: &str = "low";

const GUIDE_STATUS_PENDING: &str = "pending";
const GUIDE_STATUS_QUEUED: &str = "queued";
const GUIDE_STATUS_SENT: &str = "sent";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskTodo {
    pub id: String,
    pub task_id: String,
    pub text: String,
    pub done: bool,
    pub order: i64,
    /// "lilia" | "agent"
    pub source: String,
    pub priority: String,
    pub guide_status: Option<String>,
    pub attachments: Vec<JsonValue>,
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
    #[serde(default)]
    pub priority: Option<String>,
}

impl AgentTodoItem {
    fn is_done(&self) -> bool {
        self.completed.unwrap_or(false)
            || self.done.unwrap_or(false)
            || self.status.eq_ignore_ascii_case("completed")
    }

    fn normalized_priority(&self) -> String {
        normalize_priority(self.priority.as_deref())
    }
}

fn normalize_priority(value: Option<&str>) -> String {
    match value.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        PRIORITY_HIGH => PRIORITY_HIGH.to_string(),
        PRIORITY_LOW => PRIORITY_LOW.to_string(),
        _ => PRIORITY_NORMAL.to_string(),
    }
}

fn normalize_guide_status(value: Option<&str>) -> Option<String> {
    match value.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        GUIDE_STATUS_PENDING => Some(GUIDE_STATUS_PENDING.to_string()),
        GUIDE_STATUS_QUEUED => Some(GUIDE_STATUS_QUEUED.to_string()),
        GUIDE_STATUS_SENT => Some(GUIDE_STATUS_SENT.to_string()),
        _ => None,
    }
}

fn parse_attachments_json(value: String) -> Vec<JsonValue> {
    serde_json::from_str::<Vec<JsonValue>>(&value).unwrap_or_default()
}

fn attachments_json(attachments: Option<Vec<JsonValue>>) -> Result<String, String> {
    serde_json::to_string(&attachments.unwrap_or_default())
        .map_err(|e| format!("todo attachments: serialize 失败：{e}"))
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
                    priority: None,
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
        priority: row.get(6)?,
        guide_status: row.get(7)?,
        attachments: parse_attachments_json(row.get(8)?),
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn select_by_task(conn: &Connection, task_id: &str) -> Result<Vec<TaskTodo>, String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT id, task_id, text, done, "order", source, priority, guide_status, attachments_json, created_at, updated_at
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

fn emit_todo_changed<R: Runtime>(app: &AppHandle<R>, task_id: &str) -> Result<(), String> {
    app.emit(
        "todo-changed",
        serde_json::json!({
            "taskId": task_id,
        }),
    )
    .map_err(|err| format!("todo-changed emit failed: {err}"))
}

pub(crate) fn set_lilia_guide_status<R: Runtime>(
    app: &AppHandle<R>,
    store: &LiliaStore,
    id: &str,
    status: &str,
) -> Result<(), String> {
    let conn = store.conn()?;
    let normalized = normalize_guide_status(Some(status))
        .ok_or_else(|| format!("todo guide status: 无效状态 {status}"))?;
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT task_id, guide_status FROM task_todos WHERE id = ?1 AND source = 'lilia'",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("todo guide status: 查询失败：{e}"))?;
    let Some((task_id, current)) = row else {
        return Ok(());
    };
    if current.as_deref() == Some(normalized.as_str()) {
        return Ok(());
    }
    conn.execute(
        "UPDATE task_todos SET guide_status = ?1, updated_at = ?2 WHERE id = ?3 AND source = 'lilia'",
        params![normalized, now_millis(), id],
    )
    .map_err(|e| format!("todo guide status: 更新失败：{e}"))?;
    emit_todo_changed(app, &task_id)
}

#[tauri::command]
pub fn todo_list(task_id: String, store: State<'_, LiliaStore>) -> Result<Vec<TaskTodo>, String> {
    let conn = store.conn()?;
    select_by_task(&conn, &task_id)
}

#[tauri::command]
pub fn todo_create<R: Runtime>(
    task_id: String,
    text: String,
    priority: Option<String>,
    attachments: Option<Vec<JsonValue>>,
    app: AppHandle<R>,
    store: State<'_, LiliaStore>,
) -> Result<TaskTodo, String> {
    let conn = store.conn()?;
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("todo_create: text 不能为空".to_string());
    }
    let id = Uuid::new_v4().to_string();
    let order = next_order(&conn, &task_id)?;
    let now = now_millis();
    let attachment_values = attachments.unwrap_or_default();
    let attachment_json = attachments_json(Some(attachment_values.clone()))?;
    let todo = TaskTodo {
        id: id.clone(),
        task_id: task_id.clone(),
        text,
        done: false,
        order,
        source: "lilia".to_string(),
        priority: normalize_priority(priority.as_deref()),
        guide_status: Some(GUIDE_STATUS_PENDING.to_string()),
        attachments: attachment_values,
        created_at: now,
        updated_at: now,
    };
    conn.execute(
        r#"INSERT INTO task_todos
           (id, task_id, text, done, "order", source, priority, guide_status, attachments_json, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
        params![
            todo.id,
            todo.task_id,
            todo.text,
            if todo.done { 1 } else { 0 },
            todo.order,
            todo.source,
            todo.priority,
            todo.guide_status,
            attachment_json,
            todo.created_at,
            todo.updated_at,
        ],
    )
    .map_err(|e| format!("todo_create: insert 失败：{e}"))?;
    emit_todo_changed(&app, &task_id)?;
    Ok(todo)
}

#[tauri::command]
pub fn todo_update<R: Runtime>(
    id: String,
    text: Option<String>,
    done: Option<bool>,
    order: Option<i64>,
    priority: Option<String>,
    guide_status: Option<String>,
    app: AppHandle<R>,
    store: State<'_, LiliaStore>,
) -> Result<(), String> {
    if text.is_none()
        && done.is_none()
        && order.is_none()
        && priority.is_none()
        && guide_status.is_none()
    {
        return Ok(());
    }
    let conn = store.conn()?;
    let now = now_millis();
    let task_id: Option<String> = conn
        .query_row(
            "SELECT task_id FROM task_todos WHERE id = ?1 AND source = 'lilia'",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("todo_update: 查询 task_id 失败：{e}"))?;
    let Some(task_id) = task_id else {
        return Ok(());
    };
    // Lilia 引导由用户管理；agent 原生 Todo 镜像只能由 TodoWrite/todo_list 刷新。
    if let Some(t) = text {
        let t = t.trim().to_string();
        if t.is_empty() {
            return Err("todo_update(text): text 不能为空".to_string());
        }
        conn.execute(
            "UPDATE task_todos SET text = ?1, updated_at = ?2 WHERE id = ?3 AND source = 'lilia'",
            params![t, now, id],
        )
        .map_err(|e| format!("todo_update(text): {e}"))?;
    }
    if let Some(d) = done {
        conn.execute(
            "UPDATE task_todos SET done = ?1, updated_at = ?2 WHERE id = ?3 AND source = 'lilia'",
            params![if d { 1 } else { 0 }, now, id],
        )
        .map_err(|e| format!("todo_update(done): {e}"))?;
    }
    if let Some(o) = order {
        conn.execute(
            r#"UPDATE task_todos SET "order" = ?1, updated_at = ?2 WHERE id = ?3 AND source = 'lilia'"#,
            params![o, now, id],
        )
        .map_err(|e| format!("todo_update(order): {e}"))?;
    }
    if let Some(p) = priority {
        conn.execute(
            "UPDATE task_todos SET priority = ?1, updated_at = ?2 WHERE id = ?3 AND source = 'lilia'",
            params![normalize_priority(Some(&p)), now, id],
        )
        .map_err(|e| format!("todo_update(priority): {e}"))?;
    }
    if let Some(s) = guide_status {
        let normalized = normalize_guide_status(Some(&s))
            .ok_or_else(|| format!("todo_update(guideStatus): 无效状态 {s}"))?;
        conn.execute(
            "UPDATE task_todos SET guide_status = ?1, updated_at = ?2 WHERE id = ?3 AND source = 'lilia'",
            params![normalized, now, id],
        )
        .map_err(|e| format!("todo_update(guideStatus): {e}"))?;
    }
    emit_todo_changed(&app, &task_id)?;
    Ok(())
}

#[tauri::command]
pub fn todo_delete<R: Runtime>(
    id: String,
    app: AppHandle<R>,
    store: State<'_, LiliaStore>,
) -> Result<(), String> {
    let conn = store.conn()?;
    let task_id: Option<String> = conn
        .query_row(
            "SELECT task_id FROM task_todos WHERE id = ?1 AND source = 'lilia'",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("todo_delete: 查询 task_id 失败：{e}"))?;
    conn.execute(
        "DELETE FROM task_todos WHERE id = ?1 AND source = 'lilia'",
        params![id],
    )
    .map_err(|e| format!("todo_delete: {e}"))?;
    if let Some(task_id) = task_id {
        emit_todo_changed(&app, &task_id)?;
    }
    Ok(())
}

/// 把标准化 agent todo 列表落库。复用规则：
/// - 按 `text` 等值匹配现有 `source="agent"` 行；命中则保留 id、更新 done/order/updatedAt
/// - 未命中的新条目以 `source="agent"` 插入
/// - 本次没出现的 agent 行删除（保持 agent 列表 = 最新 SDK 状态）
/// - `source="lilia"` 的行原样保留，order 维持
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

    // 计算 Lilia 引导已经占走的 max(order)，给本次 agent 顺序作为起点的兜底。
    // 仍然给 agent 一个独立 order 段（接 Lilia 末尾递增），避免重排引导行。
    let lilia_max: i64 = conn
        .query_row(
            r#"SELECT COALESCE(MAX("order"), -1) FROM task_todos
               WHERE task_id = ?1 AND source = 'lilia'"#,
            params![task_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("apply_agent_event: 查 lilia_max 失败：{e}"))?;

    let mut seen_texts: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (idx, item) in todos.iter().enumerate() {
        let text = item.content.trim().to_string();
        if text.is_empty() {
            continue;
        }
        let done = item.is_done();
        let priority = item.normalized_priority();
        let order = lilia_max + 1 + (idx as i64);
        seen_texts.insert(text.clone());

        if let Some((id, _old_order, _created)) = existing.get(&text) {
            conn.execute(
                r#"UPDATE task_todos
                   SET done = ?1, "order" = ?2, priority = ?3, guide_status = NULL, updated_at = ?4
                   WHERE id = ?5"#,
                params![if done { 1 } else { 0 }, order, priority, now, id],
            )
            .map_err(|e| format!("apply_agent_event: update 失败：{e}"))?;
        } else {
            let id = Uuid::new_v4().to_string();
            conn.execute(
                r#"INSERT INTO task_todos
                   (id, task_id, text, done, "order", source, priority, guide_status, attachments_json, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, 'agent', ?6, NULL, '[]', ?7, ?8)"#,
                params![id, task_id, text, if done { 1 } else { 0 }, order, priority, now, now],
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
pub fn todo_apply_agent_event<R: Runtime>(
    task_id: String,
    todos: Vec<AgentTodoItem>,
    app: AppHandle<R>,
    store: State<'_, LiliaStore>,
) -> Result<Vec<TaskTodo>, String> {
    let conn = store.conn()?;
    let updated = apply_agent_event_impl(&conn, &task_id, &todos)?;
    emit_todo_changed(&app, &task_id)?;
    Ok(updated)
}
