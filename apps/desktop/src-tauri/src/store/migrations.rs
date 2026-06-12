use rusqlite::Connection;

pub(super) const RESET_BASELINE_SCHEMA_VERSION: i64 = 3;

pub(super) struct SchemaMigration {
    pub version: i64,
    pub name: &'static str,
    pub apply: fn(&Connection) -> Result<(), String>,
}

pub(super) const SCHEMA_MIGRATIONS: &[SchemaMigration] = &[
    SchemaMigration {
        version: 4,
        name: "todo_guides",
        apply: migrate_todo_guides,
    },
    SchemaMigration {
        version: 5,
        name: "todo_attachments",
        apply: migrate_todo_attachments,
    },
    SchemaMigration {
        version: 6,
        name: "task_list_indexes",
        apply: migrate_task_list_indexes,
    },
    SchemaMigration {
        version: 7,
        name: "task_title_source",
        apply: migrate_task_title_source,
    },
    SchemaMigration {
        version: 8,
        name: "task_agent_sessions",
        apply: migrate_task_agent_sessions,
    },
    SchemaMigration {
        version: 9,
        name: "task_agent_sessions_runtime_channel",
        apply: migrate_task_agent_sessions_runtime_channel,
    },
    SchemaMigration {
        version: 10,
        name: "task_runtime_states",
        apply: migrate_task_runtime_states,
    },
    SchemaMigration {
        version: 11,
        name: "task_runtime_states_process_session",
        apply: migrate_task_runtime_states_process_session,
    },
    SchemaMigration {
        version: 12,
        name: "task_runtime_control_events",
        apply: migrate_task_runtime_control_events,
    },
    SchemaMigration {
        version: 13,
        name: "task_pending_turns",
        apply: migrate_task_pending_turns,
    },
    SchemaMigration {
        version: 14,
        name: "task_pending_turns_runtime_channel",
        apply: migrate_task_pending_turns_runtime_channel,
    },
    SchemaMigration {
        version: 15,
        name: "task_runtime_finalizations",
        apply: migrate_task_runtime_finalizations,
    },
    SchemaMigration {
        version: 16,
        name: "task_runtime_states_context_json",
        apply: migrate_task_runtime_states_context_json,
    },
    SchemaMigration {
        version: 17,
        name: "global_automations",
        apply: migrate_global_automations,
    },
];

fn migrate_todo_guides(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE task_todos_next (
          id           TEXT PRIMARY KEY,
          task_id      TEXT NOT NULL,
          text         TEXT NOT NULL,
          done         INTEGER NOT NULL DEFAULT 0,
          "order"      INTEGER NOT NULL,
          source       TEXT NOT NULL CHECK (source IN ('lilia','agent')),
          priority     TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('high','normal','low')),
          guide_status TEXT CHECK (guide_status IS NULL OR guide_status IN ('pending','queued','sent')),
          created_at   INTEGER NOT NULL,
          updated_at   INTEGER NOT NULL
        );

        INSERT INTO task_todos_next
          (id, task_id, text, done, "order", source, priority, guide_status, created_at, updated_at)
        SELECT
          id,
          task_id,
          text,
          done,
          "order",
          CASE source WHEN 'user' THEN 'lilia' ELSE source END,
          'normal',
          CASE source WHEN 'agent' THEN NULL ELSE 'pending' END,
          created_at,
          updated_at
        FROM task_todos;

        DROP TABLE task_todos;
        ALTER TABLE task_todos_next RENAME TO task_todos;
        CREATE INDEX idx_task_todos_task_id_order
          ON task_todos(task_id, "order");
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 todo_guides 失败：{e}"))
}

fn migrate_todo_attachments(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        ALTER TABLE task_todos
          ADD COLUMN attachments_json TEXT NOT NULL DEFAULT '[]';
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 todo_attachments 失败：{e}"))
}

fn migrate_task_list_indexes(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_tasks_project_archived_order
          ON tasks(project_id, archived, pinned DESC, sort_order ASC);
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 task_list_indexes 失败：{e}"))
}

fn migrate_task_title_source(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        ALTER TABLE tasks
          ADD COLUMN title_source TEXT NOT NULL DEFAULT 'auto'
          CHECK (title_source IN ('auto','manual'));
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 task_title_source 失败：{e}"))
}

fn migrate_task_agent_sessions(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE task_agent_sessions (
          task_id         TEXT NOT NULL,
          backend         TEXT NOT NULL CHECK (backend IN ('claude','codex')),
          runtime_channel TEXT NOT NULL DEFAULT 'builtin'
                          CHECK (runtime_channel IN ('builtin','nanobot')),
          session_id      TEXT NOT NULL,
          updated_at      INTEGER NOT NULL,
          PRIMARY KEY (task_id, backend, runtime_channel),
          FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
        );
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 task_agent_sessions 失败：{e}"))
}

fn migrate_task_agent_sessions_runtime_channel(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE task_agent_sessions_next (
          task_id         TEXT NOT NULL,
          backend         TEXT NOT NULL CHECK (backend IN ('claude','codex')),
          runtime_channel TEXT NOT NULL DEFAULT 'builtin'
                          CHECK (runtime_channel IN ('builtin','nanobot')),
          session_id      TEXT NOT NULL,
          updated_at      INTEGER NOT NULL,
          PRIMARY KEY (task_id, backend, runtime_channel),
          FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
        );

        INSERT INTO task_agent_sessions_next
          (task_id, backend, runtime_channel, session_id, updated_at)
        SELECT task_id, backend, 'builtin', session_id, updated_at
        FROM task_agent_sessions;

        DROP TABLE task_agent_sessions;
        ALTER TABLE task_agent_sessions_next RENAME TO task_agent_sessions;
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 task_agent_sessions runtime_channel 失败：{e}"))
}

fn migrate_task_runtime_states(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE task_runtime_states (
          task_id         TEXT PRIMARY KEY,
          turn_id         TEXT NOT NULL,
          backend         TEXT NOT NULL CHECK (backend IN ('claude','codex')),
          runtime_channel TEXT NOT NULL CHECK (runtime_channel IN ('builtin','nanobot')),
          phase           TEXT NOT NULL CHECK (phase IN
                            ('running','interrupted_pending_finish','reset_pending_finish')),
          process_session_id TEXT,
          runtime_epoch   TEXT NOT NULL,
          context_json    TEXT,
          updated_at      INTEGER NOT NULL,
          FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
        );

        CREATE INDEX idx_task_runtime_states_epoch_backend
          ON task_runtime_states(runtime_epoch, backend, updated_at);
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 task_runtime_states 失败：{e}"))
}

fn migrate_task_runtime_states_process_session(conn: &Connection) -> Result<(), String> {
    let has_column = conn
        .prepare("PRAGMA table_info(task_runtime_states)")
        .and_then(|mut stmt| {
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            let mut has = false;
            for row in rows {
                if row? == "process_session_id" {
                    has = true;
                    break;
                }
            }
            Ok(has)
        })
        .map_err(|e| {
            format!("lilia-store: 检查 task_runtime_states process_session_id 失败：{e}")
        })?;
    if has_column {
        return Ok(());
    }
    conn.execute_batch(
        r#"
        ALTER TABLE task_runtime_states
          ADD COLUMN process_session_id TEXT;
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 task_runtime_states process_session_id 失败：{e}"))
}

fn migrate_task_runtime_control_events(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE task_runtime_control_events (
          id              INTEGER PRIMARY KEY AUTOINCREMENT,
          task_id         TEXT NOT NULL,
          name            TEXT NOT NULL,
          attributes_json TEXT NOT NULL DEFAULT '{}',
          payload_json    TEXT,
          created_at      INTEGER NOT NULL,
          FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
        );

        CREATE INDEX idx_task_runtime_control_events_task_id
          ON task_runtime_control_events(task_id, id);
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 task_runtime_control_events 失败：{e}"))
}

fn migrate_task_pending_turns(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE task_pending_turns (
          id              INTEGER PRIMARY KEY AUTOINCREMENT,
          task_id         TEXT NOT NULL,
          content         TEXT NOT NULL,
          composer_json   TEXT NOT NULL,
          project_cwd     TEXT NOT NULL,
          attachments_json TEXT NOT NULL DEFAULT '[]',
          workflow_json   TEXT,
          message_json    TEXT NOT NULL,
          turn_id         TEXT NOT NULL,
          runtime_channel TEXT NOT NULL DEFAULT 'builtin'
                          CHECK (runtime_channel IN ('builtin','nanobot')),
          guide_id        TEXT,
          created_at      INTEGER NOT NULL,
          FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
        );

        CREATE INDEX idx_task_pending_turns_task_id
          ON task_pending_turns(task_id, id);
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 task_pending_turns 失败：{e}"))
}

fn migrate_task_pending_turns_runtime_channel(conn: &Connection) -> Result<(), String> {
    let has_column = conn
        .prepare("PRAGMA table_info(task_pending_turns)")
        .and_then(|mut stmt| {
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            let mut has = false;
            for row in rows {
                if row? == "runtime_channel" {
                    has = true;
                    break;
                }
            }
            Ok(has)
        })
        .map_err(|e| format!("lilia-store: 检查 task_pending_turns runtime_channel 失败：{e}"))?;
    if has_column {
        return Ok(());
    }
    conn.execute_batch(
        r#"
        ALTER TABLE task_pending_turns
          ADD COLUMN runtime_channel TEXT NOT NULL DEFAULT 'builtin'
                      CHECK (runtime_channel IN ('builtin','nanobot'));
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 task_pending_turns runtime_channel 失败：{e}"))
}

fn migrate_task_runtime_finalizations(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE task_runtime_finalizations (
          task_id               TEXT PRIMARY KEY,
          pending_reset_cleanup INTEGER NOT NULL DEFAULT 0
                                CHECK (pending_reset_cleanup IN (0, 1)),
          rollback_json         TEXT,
          updated_at            INTEGER NOT NULL,
          FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
        );
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 task_runtime_finalizations 失败：{e}"))
}

fn migrate_task_runtime_states_context_json(conn: &Connection) -> Result<(), String> {
    let has_column = conn
        .prepare("PRAGMA table_info(task_runtime_states)")
        .and_then(|mut stmt| {
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            let mut has = false;
            for row in rows {
                if row? == "context_json" {
                    has = true;
                    break;
                }
            }
            Ok(has)
        })
        .map_err(|e| format!("lilia-store: 检查 task_runtime_states context_json 失败：{e}"))?;
    if has_column {
        return Ok(());
    }
    conn.execute_batch(
        r#"
        ALTER TABLE task_runtime_states
          ADD COLUMN context_json TEXT;
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 task_runtime_states context_json 失败：{e}"))
}

fn migrate_global_automations(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE automation_workflows (
          id                   TEXT PRIMARY KEY,
          name                 TEXT NOT NULL,
          enabled              INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
          scope_json           TEXT NOT NULL DEFAULT '{}',
          draft_json           TEXT NOT NULL DEFAULT '{"nodes":[],"edges":[],"scope":{}}',
          published_version_id TEXT,
          created_at           INTEGER NOT NULL,
          updated_at           INTEGER NOT NULL
        );

        CREATE TABLE automation_workflow_versions (
          id            TEXT PRIMARY KEY,
          workflow_id   TEXT NOT NULL,
          version       INTEGER NOT NULL,
          snapshot_json TEXT NOT NULL,
          created_at    INTEGER NOT NULL,
          FOREIGN KEY (workflow_id) REFERENCES automation_workflows(id) ON DELETE CASCADE
        );

        CREATE UNIQUE INDEX idx_automation_workflow_versions_workflow_version
          ON automation_workflow_versions(workflow_id, version);

        CREATE TABLE automation_runs (
          id                  TEXT PRIMARY KEY,
          workflow_id         TEXT NOT NULL,
          workflow_version_id TEXT NOT NULL,
          status              TEXT NOT NULL CHECK (status IN
                                ('pending','running','succeeded','failed','skipped','waiting_user')),
          trigger_json        TEXT NOT NULL,
          scope_json          TEXT NOT NULL,
          started_at          INTEGER NOT NULL,
          finished_at         INTEGER,
          error               TEXT,
          FOREIGN KEY (workflow_id) REFERENCES automation_workflows(id) ON DELETE CASCADE,
          FOREIGN KEY (workflow_version_id) REFERENCES automation_workflow_versions(id)
        );

        CREATE INDEX idx_automation_runs_workflow_started
          ON automation_runs(workflow_id, started_at DESC);
        CREATE INDEX idx_automation_runs_status
          ON automation_runs(status);

        CREATE TABLE automation_run_nodes (
          id          TEXT PRIMARY KEY,
          run_id      TEXT NOT NULL,
          node_id     TEXT NOT NULL,
          status      TEXT NOT NULL CHECK (status IN
                        ('pending','running','succeeded','failed','skipped','waiting_user')),
          input_json  TEXT NOT NULL DEFAULT '{}',
          output_json TEXT,
          error       TEXT,
          started_at  INTEGER,
          finished_at INTEGER,
          FOREIGN KEY (run_id) REFERENCES automation_runs(id) ON DELETE CASCADE
        );

        CREATE INDEX idx_automation_run_nodes_run
          ON automation_run_nodes(run_id);
        "#,
    )
    .map_err(|e| format!("lilia-store: 迁移 global_automations 失败：{e}"))
}

pub(super) fn ensure_schema_with_migrations(
    conn: &mut Connection,
    target_version: i64,
    migrations: &[SchemaMigration],
) -> Result<(), String> {
    let current: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| format!("lilia-store: 读取 user_version 失败：{e}"))?;

    if current < RESET_BASELINE_SCHEMA_VERSION || current > target_version {
        super::schema::reset_development_schema(conn)?;
    }

    let mut version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| format!("lilia-store: 读取重置后 user_version 失败：{e}"))?;

    for migration in migrations {
        if migration.version <= version {
            continue;
        }
        if migration.version != version + 1 {
            return Err(format!(
                "lilia-store: schema migration {} 版本不连续，当前 {version}",
                migration.name
            ));
        }
        (migration.apply)(conn)?;
        conn.execute_batch(&format!("PRAGMA user_version = {};", migration.version))
            .map_err(|e| format!("lilia-store: 写 {} 版本失败：{e}", migration.name))?;
        version = migration.version;
    }

    Ok(())
}
