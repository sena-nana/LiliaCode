use rusqlite::{params, OptionalExtension};
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Runtime, State};
use uuid::Uuid;

use crate::store::LiliaStore;
use crate::util::now_millis;

use super::agent_sync::apply_agent_event_impl;
use super::repository::{attachments_json, emit_todo_changed, next_order, select_by_task};
use super::types::{
    normalize_guide_status, normalize_priority, AgentTodoItem, TaskTodo, GUIDE_STATUS_PENDING,
};

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
    let next_text = text
        .map(|t| {
            let t = t.trim().to_string();
            if t.is_empty() {
                return Err("todo_update(text): text 不能为空".to_string());
            }
            Ok(t)
        })
        .transpose()?;
    let next_done = done.map(|d| if d { 1i64 } else { 0i64 });
    let next_priority = priority.map(|p| normalize_priority(Some(&p)));
    let next_guide_status = guide_status
        .map(|s| {
            normalize_guide_status(Some(&s))
                .ok_or_else(|| format!("todo_update(guideStatus): 无效状态 {s}"))
        })
        .transpose()?;
    conn.execute(
        r#"UPDATE task_todos
           SET text = COALESCE(?1, text),
               done = COALESCE(?2, done),
               "order" = COALESCE(?3, "order"),
               priority = COALESCE(?4, priority),
               guide_status = COALESCE(?5, guide_status),
               updated_at = ?6
           WHERE id = ?7 AND source = 'lilia'"#,
        params![
            next_text,
            next_done,
            order,
            next_priority,
            next_guide_status,
            now,
            id
        ],
    )
    .map_err(|e| format!("todo_update: {e}"))?;
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
