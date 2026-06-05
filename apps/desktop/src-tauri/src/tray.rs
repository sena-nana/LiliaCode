use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use rusqlite::params;
use tauri::menu::{Menu, MenuBuilder, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::popup_windows;
use crate::store::LiliaStore;

const TRAY_ID: &str = "lilia-main-tray";
const MENU_OPEN_MAIN: &str = "tray:open-main";
const MENU_QUIT: &str = "tray:quit";
const MENU_EMPTY_RECENT: &str = "tray:recent-empty";
const RECENT_MENU_PREFIX: &str = "tray:recent:";
const SIDEBAR_CONVERSATION_LIMIT: usize = 8;
const RECENT_TITLE_MAX_CHARS: usize = 32;
const SINGLE_CLICK_DELAY: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
struct SidebarConversation {
    task_id: String,
    project_id: Option<String>,
    title: String,
}

#[derive(Clone)]
struct TrayMenuState {
    recent_menu: Submenu<Wry>,
    conversations_by_menu_id: Arc<Mutex<HashMap<String, SidebarConversation>>>,
}

pub(crate) fn setup_tray(app: &AppHandle) -> Result<(), String> {
    let menu_state = build_tray_menu(app)?;
    append_empty_sidebar_conversations_item(app, &menu_state)?;
    app.manage(menu_state.clone());

    let click_seq = Arc::new(AtomicU64::new(0));
    let tray_app = app.clone();
    let tray_click_seq = Arc::clone(&click_seq);
    let menu_click_seq = Arc::clone(&click_seq);

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&build_root_menu(app, &menu_state.recent_menu)?)
        .tooltip("LiliaCode")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(move |_tray, event| {
            handle_tray_icon_event(&tray_app, &tray_click_seq, event);
        })
        .on_menu_event(move |app, event| {
            handle_menu_event(app, &menu_click_seq, event.id().as_ref());
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder
        .build(app)
        .map_err(|e| format!("创建系统托盘失败：{e}"))?;
    Ok(())
}

fn build_tray_menu(app: &AppHandle) -> Result<TrayMenuState, String> {
    let recent_menu = Submenu::with_id(app, "tray:recent", "最近对话", true)
        .map_err(|e| format!("创建侧边栏对话菜单失败：{e}"))?;
    Ok(TrayMenuState {
        recent_menu,
        conversations_by_menu_id: Arc::new(Mutex::new(HashMap::new())),
    })
}

fn build_root_menu(app: &AppHandle, recent_menu: &Submenu<Wry>) -> Result<Menu<Wry>, String> {
    let open_main = MenuItem::with_id(app, MENU_OPEN_MAIN, "打开主窗口", true, None::<&str>)
        .map_err(|e| format!("创建打开主窗口菜单失败：{e}"))?;
    let separator =
        PredefinedMenuItem::separator(app).map_err(|e| format!("创建托盘菜单分隔线失败：{e}"))?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出应用", true, None::<&str>)
        .map_err(|e| format!("创建退出菜单失败：{e}"))?;
    MenuBuilder::new(app)
        .item(&open_main)
        .item(recent_menu)
        .item(&separator)
        .item(&quit)
        .build()
        .map_err(|e| format!("创建托盘菜单失败：{e}"))
}

fn refresh_sidebar_conversations_menu(app: &AppHandle) -> Result<(), String> {
    let Some(state) = app.try_state::<TrayMenuState>() else {
        return Ok(());
    };
    refresh_sidebar_conversations_menu_with_state(app, &state)
}

fn refresh_sidebar_conversations_menu_with_state(
    app: &AppHandle,
    state: &TrayMenuState,
) -> Result<(), String> {
    clear_sidebar_conversations_menu(state)?;
    let mut conversations_by_menu_id = HashMap::new();
    let conversations = load_sidebar_conversations(app)?;
    if conversations.is_empty() {
        replace_conversation_cache(state, conversations_by_menu_id)?;
        append_empty_sidebar_conversations_item(app, state)?;
        return Ok(());
    }
    for conversation in conversations {
        let item_id = format!("{RECENT_MENU_PREFIX}{}", conversation.task_id);
        let label = recent_menu_label(&conversation.title);
        let item = MenuItem::with_id(app, &item_id, label, true, None::<&str>)
            .map_err(|e| format!("创建侧边栏对话菜单项失败：{e}"))?;
        state
            .recent_menu
            .append(&item)
            .map_err(|e| format!("更新侧边栏对话菜单失败：{e}"))?;
        conversations_by_menu_id.insert(item_id, conversation);
    }
    replace_conversation_cache(state, conversations_by_menu_id)?;
    Ok(())
}

fn clear_sidebar_conversations_menu(state: &TrayMenuState) -> Result<(), String> {
    let existing_count = state
        .recent_menu
        .items()
        .map_err(|e| format!("读取侧边栏对话菜单失败：{e}"))?
        .len();
    for index in (0..existing_count).rev() {
        state
            .recent_menu
            .remove_at(index)
            .map_err(|e| format!("清空侧边栏对话菜单失败：{e}"))?;
    }
    Ok(())
}

fn replace_conversation_cache(
    state: &TrayMenuState,
    conversations_by_menu_id: HashMap<String, SidebarConversation>,
) -> Result<(), String> {
    let mut cache = state
        .conversations_by_menu_id
        .lock()
        .map_err(|_| "更新侧边栏对话缓存失败".to_string())?;
    *cache = conversations_by_menu_id;
    Ok(())
}

fn append_empty_sidebar_conversations_item(
    app: &AppHandle,
    state: &TrayMenuState,
) -> Result<(), String> {
    let empty = MenuItem::with_id(app, MENU_EMPTY_RECENT, "暂无最近对话", false, None::<&str>)
        .map_err(|e| format!("创建空侧边栏对话菜单失败：{e}"))?;
    state
        .recent_menu
        .append(&empty)
        .map_err(|e| format!("更新侧边栏对话菜单失败：{e}"))?;
    Ok(())
}

fn load_sidebar_conversations(app: &AppHandle) -> Result<Vec<SidebarConversation>, String> {
    let Some(store) = app.try_state::<LiliaStore>() else {
        return Ok(Vec::new());
    };
    let conn = store.conn()?;
    let mut stmt = conn
        .prepare(
            r#"SELECT id, project_id, title
               FROM (
                 SELECT t.id,
                        t.project_id,
                        t.title,
                        CASE WHEN t.project_id IS NULL THEN 1 ELSE 0 END AS project_group,
                        COALESCE(p.pinned, 0) AS project_pinned,
                        COALESCE(p.sort_order, 0) AS project_sort_order,
                        t.pinned AS task_pinned,
                        t.sort_order AS task_sort_order
                 FROM tasks t
                 LEFT JOIN projects p ON p.id = t.project_id
                 WHERE t.archived = 0
               )
               ORDER BY project_group ASC,
                        project_pinned DESC,
                        project_sort_order ASC,
                        project_id ASC,
                        task_pinned DESC,
                        task_sort_order ASC,
                        id ASC
               LIMIT ?1"#,
        )
        .map_err(|e| format!("读取侧边栏对话: prepare 失败：{e}"))?;
    let rows = stmt
        .query_map(params![SIDEBAR_CONVERSATION_LIMIT as i64], |row| {
            Ok(SidebarConversation {
                task_id: row.get(0)?,
                project_id: row.get(1)?,
                title: row.get(2)?,
            })
        })
        .map_err(|e| format!("读取侧边栏对话: query 失败：{e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("读取侧边栏对话: row 失败：{e}"))?);
    }
    Ok(out)
}

fn recent_menu_label(title: &str) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return "未命名对话".to_string();
    }
    let mut out = String::new();
    for (index, ch) in trimmed.chars().enumerate() {
        if index >= RECENT_TITLE_MAX_CHARS {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}

fn handle_tray_icon_event(app: &AppHandle, click_seq: &Arc<AtomicU64>, event: TrayIconEvent) {
    match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } => {
            let seq = click_seq.fetch_add(1, Ordering::SeqCst) + 1;
            let app = app.clone();
            let click_seq = Arc::clone(click_seq);
            tauri::async_runtime::spawn_blocking(move || {
                std::thread::sleep(SINGLE_CLICK_DELAY);
                if click_seq.load(Ordering::SeqCst) != seq {
                    return;
                }
                if let Err(err) = popup_windows::open_popup_for_shortcut(&app) {
                    eprintln!("[tray] open popup failed: {err}");
                }
            });
        }
        TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        } => {
            click_seq.fetch_add(1, Ordering::SeqCst);
            if let Err(err) = open_main_window(app) {
                eprintln!("[tray] open main failed: {err}");
            }
        }
        TrayIconEvent::Click {
            button: MouseButton::Right,
            button_state: MouseButtonState::Down,
            ..
        } => {
            if let Err(err) = refresh_sidebar_conversations_menu(app) {
                eprintln!("[tray] refresh sidebar conversations failed: {err}");
            }
        }
        _ => {}
    }
}

fn handle_menu_event(app: &AppHandle, click_seq: &Arc<AtomicU64>, id: &str) {
    click_seq.fetch_add(1, Ordering::SeqCst);

    match id {
        MENU_OPEN_MAIN => {
            if let Err(err) = open_main_window(app) {
                eprintln!("[tray] open main failed: {err}");
            }
        }
        MENU_QUIT => app.exit(0),
        MENU_EMPTY_RECENT => {}
        _ if id.starts_with(RECENT_MENU_PREFIX) => match cached_sidebar_conversation(app, id) {
            Ok(Some(conversation)) => {
                let app = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    if let Err(err) = popup_windows::open_task_window(
                        &app,
                        conversation.project_id,
                        conversation.task_id,
                    ) {
                        eprintln!("[tray] open recent conversation failed: {err}");
                    }
                });
            }
            Ok(None) => {}
            Err(err) => eprintln!("[tray] read sidebar conversation cache failed: {err}"),
        },
        _ => {}
    }
}

fn open_main_window(app: &AppHandle) -> Result<(), String> {
    popup_windows::popup_focus_main(app.clone(), "/".to_string())
}

fn cached_sidebar_conversation(
    app: &AppHandle,
    menu_id: &str,
) -> Result<Option<SidebarConversation>, String> {
    let Some(state) = app.try_state::<TrayMenuState>() else {
        return Ok(None);
    };
    state
        .conversations_by_menu_id
        .lock()
        .map_err(|_| "读取侧边栏对话缓存失败".to_string())
        .map(|cache| cache.get(menu_id).cloned())
}
