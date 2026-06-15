use std::collections::BTreeMap;
use std::process::Command;

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Runtime, State};

use crate::agent_timeline::AgentTimelineEvent;
use crate::chat::runner::locate_agent_runner;
use crate::provider::{
    build_codex_app_server_probe_status, resolve_connection_for, validate_backend_ready_for_send,
    ConnectionMode,
};
use crate::store::LiliaStore;
use crate::util::now_millis;
use crate::{BACKEND_CLAUDE, BACKEND_CODEX};

const DAY_MS: i64 = 86_400_000;
const DEFAULT_DAYS: i64 = 7;
const RECENT_LIMIT: i64 = 20;

#[derive(Debug, Clone)]
struct UsageRecord {
    event_id: String,
    task_id: String,
    turn_id: Option<String>,
    backend: String,
    session_id: Option<String>,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_creation_tokens: i64,
    total_tokens: i64,
    known_cost_usd: Option<f64>,
    raw_usage_json: String,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaUsageStatsInput {
    pub days: Option<i64>,
    pub backend: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaUsageTokenTotals {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaUsageCostCoverage {
    pub known_cost_usd: Option<f64>,
    pub cost_record_count: i64,
    pub total_record_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaUsageDailyBucket {
    pub day_start: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub total_tokens: i64,
    pub known_cost_usd: Option<f64>,
    pub cost_record_count: i64,
    pub record_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaUsageBackendSummary {
    pub backend: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub total_tokens: i64,
    pub known_cost_usd: Option<f64>,
    pub cost_record_count: i64,
    pub record_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaUsageRecentRecord {
    pub event_id: String,
    pub task_id: String,
    pub turn_id: Option<String>,
    pub backend: String,
    pub session_id: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub total_tokens: i64,
    pub known_cost_usd: Option<f64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaUsageStats {
    pub days: i64,
    pub backend: String,
    pub range_start: i64,
    pub range_end: i64,
    pub totals: QuotaUsageTokenTotals,
    pub cost: QuotaUsageCostCoverage,
    pub daily: Vec<QuotaUsageDailyBucket>,
    pub backends: Vec<QuotaUsageBackendSummary>,
    pub recent: Vec<QuotaUsageRecentRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountQuotaWindow {
    pub used_percent: f64,
    pub window_duration_mins: Option<f64>,
    pub resets_at: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountQuotaStatus {
    pub available: bool,
    pub connection_mode: String,
    pub limit_id: Option<String>,
    pub limit_name: Option<String>,
    pub plan_type: Option<String>,
    pub rate_limit_reached_type: Option<String>,
    pub five_hour: Option<CodexAccountQuotaWindow>,
    pub weekly: Option<CodexAccountQuotaWindow>,
    pub fetched_at: i64,
    pub error: Option<String>,
}

#[derive(Default)]
struct AggregatedUsage {
    totals: QuotaUsageTokenTotals,
    known_cost_usd: f64,
    cost_record_count: i64,
    record_count: i64,
}

impl AggregatedUsage {
    fn add_record(&mut self, record: &UsageRecord) {
        self.totals.input_tokens += record.input_tokens;
        self.totals.output_tokens += record.output_tokens;
        self.totals.cache_read_tokens += record.cache_read_tokens;
        self.totals.cache_creation_tokens += record.cache_creation_tokens;
        self.totals.total_tokens += record.total_tokens;
        self.record_count += 1;
        if let Some(cost) = record.known_cost_usd {
            self.known_cost_usd += cost;
            self.cost_record_count += 1;
        }
    }

    fn known_cost(&self) -> Option<f64> {
        (self.cost_record_count > 0).then_some(self.known_cost_usd)
    }

    fn token_totals(&self) -> QuotaUsageTokenTotals {
        self.totals.clone()
    }
}

fn codex_account_quota_unavailable(
    connection_mode: &str,
    error: Option<String>,
) -> CodexAccountQuotaStatus {
    CodexAccountQuotaStatus {
        available: false,
        connection_mode: connection_mode.to_string(),
        limit_id: None,
        limit_name: None,
        plan_type: None,
        rate_limit_reached_type: None,
        five_hour: None,
        weekly: None,
        fetched_at: now_millis(),
        error,
    }
}

fn locate_codex_account_quota_utility<R: Runtime>(app: &AppHandle<R>) -> std::path::PathBuf {
    let runner = locate_agent_runner(app);
    runner
        .parent()
        .map(|dir| dir.join("codex-account-quota.mjs"))
        .unwrap_or_else(|| std::path::PathBuf::from("codex-account-quota.mjs"))
}

fn run_codex_account_quota_utility<R: Runtime>(
    app: &AppHandle<R>,
    codex_path: String,
) -> CodexAccountQuotaStatus {
    let script = locate_codex_account_quota_utility(app);
    let output = match Command::new("node")
        .arg(script)
        .env("LILIA_CODEX_CLI_PATH", codex_path)
        .env_remove("OPENAI_BASE_URL")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY")
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            return codex_account_quota_unavailable(
                "codex-account",
                Some(format!("无法启动 Codex 官方额度查询：{err}")),
            );
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let Some(line) = stdout.lines().find(|line| !line.trim().is_empty()) else {
        let detail = stderr.trim();
        return codex_account_quota_unavailable(
            "codex-account",
            Some(if detail.is_empty() {
                "Codex 官方额度查询没有返回数据。".to_string()
            } else {
                format!("Codex 官方额度查询没有返回数据：{detail}")
            }),
        );
    };

    match serde_json::from_str::<CodexAccountQuotaStatus>(line) {
        Ok(mut status) => {
            status.connection_mode = "codex-account".to_string();
            if status.fetched_at <= 0 {
                status.fetched_at = now_millis();
            }
            status
        }
        Err(err) => {
            let detail = stderr.trim();
            codex_account_quota_unavailable(
                "codex-account",
                Some(if detail.is_empty() {
                    format!("解析 Codex 官方额度查询输出失败：{err}")
                } else {
                    format!("解析 Codex 官方额度查询输出失败：{err}；{detail}")
                }),
            )
        }
    }
}

fn usage_daily_bucket(day_start: i64, bucket: AggregatedUsage) -> QuotaUsageDailyBucket {
    let totals = bucket.token_totals();
    QuotaUsageDailyBucket {
        day_start,
        input_tokens: totals.input_tokens,
        output_tokens: totals.output_tokens,
        cache_read_tokens: totals.cache_read_tokens,
        cache_creation_tokens: totals.cache_creation_tokens,
        total_tokens: totals.total_tokens,
        known_cost_usd: bucket.known_cost(),
        cost_record_count: bucket.cost_record_count,
        record_count: bucket.record_count,
    }
}

fn usage_backend_summary(backend: String, summary: AggregatedUsage) -> QuotaUsageBackendSummary {
    let totals = summary.token_totals();
    QuotaUsageBackendSummary {
        backend,
        input_tokens: totals.input_tokens,
        output_tokens: totals.output_tokens,
        cache_read_tokens: totals.cache_read_tokens,
        cache_creation_tokens: totals.cache_creation_tokens,
        total_tokens: totals.total_tokens,
        known_cost_usd: summary.known_cost(),
        cost_record_count: summary.cost_record_count,
        record_count: summary.record_count,
    }
}

pub(crate) fn record_from_timeline_event(
    conn: &Connection,
    event: &AgentTimelineEvent,
) -> Result<(), String> {
    let Some(record) = extract_usage_record(event)? else {
        return Ok(());
    };
    conn.execute(
        r#"INSERT INTO agent_usage_records
           (event_id, task_id, turn_id, backend, session_id,
            input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
            total_tokens, known_cost_usd, raw_usage_json, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
           ON CONFLICT(event_id) DO UPDATE SET
             task_id               = excluded.task_id,
             turn_id               = excluded.turn_id,
             backend               = excluded.backend,
             session_id            = excluded.session_id,
             input_tokens          = excluded.input_tokens,
             output_tokens         = excluded.output_tokens,
             cache_read_tokens     = excluded.cache_read_tokens,
             cache_creation_tokens = excluded.cache_creation_tokens,
             total_tokens          = excluded.total_tokens,
             known_cost_usd        = excluded.known_cost_usd,
             raw_usage_json        = excluded.raw_usage_json,
             created_at            = excluded.created_at,
             updated_at            = excluded.updated_at"#,
        params![
            record.event_id,
            record.task_id,
            record.turn_id,
            record.backend,
            record.session_id,
            record.input_tokens,
            record.output_tokens,
            record.cache_read_tokens,
            record.cache_creation_tokens,
            record.total_tokens,
            record.known_cost_usd,
            record.raw_usage_json,
            record.created_at,
            record.updated_at,
        ],
    )
    .map(|_| ())
    .map_err(|e| format!("quota_usage: 写入 usage 记录失败：{e}"))
}

pub(crate) fn clear_task_usage(conn: &Connection, task_id: &str) -> Result<usize, String> {
    conn.execute(
        "DELETE FROM agent_usage_records WHERE task_id = ?1",
        params![task_id],
    )
    .map_err(|e| format!("quota_usage: 清理任务 usage 记录失败：{e}"))
}

fn extract_usage_record(event: &AgentTimelineEvent) -> Result<Option<UsageRecord>, String> {
    let Some(payload) = event.payload.as_object() else {
        return Ok(None);
    };
    let Some(usage) = payload.get("usage") else {
        return Ok(None);
    };
    if !usage.is_object() {
        return Ok(None);
    }

    let input_tokens = int_field(
        usage,
        &[
            "inputTokens",
            "input_tokens",
            "promptTokens",
            "prompt_tokens",
        ],
    );
    let output_tokens = int_field(
        usage,
        &[
            "outputTokens",
            "output_tokens",
            "completionTokens",
            "completion_tokens",
        ],
    );
    let cache_read_tokens = int_field(
        usage,
        &[
            "cacheReadTokens",
            "cache_read_tokens",
            "cacheReadInputTokens",
            "cache_read_input_tokens",
            "cachedInputTokens",
            "cached_input_tokens",
        ],
    );
    let cache_creation_tokens = int_field(
        usage,
        &[
            "cacheCreationTokens",
            "cache_creation_tokens",
            "cacheCreationInputTokens",
            "cache_creation_input_tokens",
        ],
    );
    let explicit_total = int_field(usage, &["totalTokens", "total_tokens"]);
    let token_sum = input_tokens + output_tokens + cache_read_tokens + cache_creation_tokens;
    let total_tokens = if explicit_total > 0 {
        explicit_total.max(token_sum)
    } else {
        token_sum
    };
    let known_cost_usd = number_field(&event.payload, &["totalCostUsd", "total_cost_usd"])
        .or_else(|| number_field(usage, &["totalCostUsd", "total_cost_usd"]))
        .or_else(|| number_field(&event.payload, &["costUsd", "cost_usd"]))
        .or_else(|| number_field(usage, &["costUsd", "cost_usd"]))
        .or_else(|| number_field(&event.payload, &["estimatedCostUsd", "estimated_cost_usd"]))
        .or_else(|| number_field(usage, &["estimatedCostUsd", "estimated_cost_usd"]));

    if total_tokens == 0 && known_cost_usd.is_none() {
        return Ok(None);
    }

    let raw_usage_json =
        serde_json::to_string(usage).map_err(|e| format!("quota_usage: usage 序列化失败：{e}"))?;
    Ok(Some(UsageRecord {
        event_id: event.id.clone(),
        task_id: event.task_id.clone(),
        turn_id: event.turn_id.clone(),
        backend: normalize_backend(&event.backend).unwrap_or_else(|| event.backend.clone()),
        session_id: string_field(&event.payload, &["sessionId", "session_id"]),
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        total_tokens,
        known_cost_usd,
        raw_usage_json,
        created_at: event.created_at,
        updated_at: event.updated_at,
    }))
}

fn int_field(value: &JsonValue, keys: &[&str]) -> i64 {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(json_int)
        .unwrap_or(0)
}

fn json_int(value: &JsonValue) -> Option<i64> {
    if let Some(n) = value.as_i64() {
        return (n > 0).then_some(n);
    }
    value
        .as_u64()
        .and_then(|n| i64::try_from(n).ok())
        .filter(|n| *n > 0)
}

fn number_field(value: &JsonValue, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(|value| value.as_f64())
        .filter(|n| n.is_finite() && *n >= 0.0)
}

fn string_field(value: &JsonValue, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
}

fn normalize_backend(value: &str) -> Option<String> {
    match value {
        BACKEND_CLAUDE => Some(BACKEND_CLAUDE.to_string()),
        BACKEND_CODEX => Some(BACKEND_CODEX.to_string()),
        _ => None,
    }
}

fn normalize_days(value: Option<i64>) -> i64 {
    match value {
        Some(30) => 30,
        _ => DEFAULT_DAYS,
    }
}

fn normalize_backend_filter(value: Option<String>) -> String {
    match value.as_deref() {
        Some(BACKEND_CLAUDE) => BACKEND_CLAUDE.to_string(),
        Some(BACKEND_CODEX) => BACKEND_CODEX.to_string(),
        _ => "all".to_string(),
    }
}

fn day_start(timestamp: i64) -> i64 {
    timestamp.div_euclid(DAY_MS) * DAY_MS
}

fn load_usage_records(
    conn: &Connection,
    range_start: i64,
    range_end: i64,
    backend: &str,
) -> Result<Vec<UsageRecord>, String> {
    let mut sql = String::from(
        r#"SELECT event_id, task_id, turn_id, backend, session_id,
                  input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
                  total_tokens, known_cost_usd, raw_usage_json, created_at, updated_at
           FROM agent_usage_records
           WHERE created_at >= ?1 AND created_at < ?2"#,
    );
    if backend != "all" {
        sql.push_str(" AND backend = ?3");
    }
    sql.push_str(" ORDER BY created_at ASC");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("quota_usage_get_stats: prepare 失败：{e}"))?;
    let mut rows = if backend == "all" {
        stmt.query(params![range_start, range_end])
    } else {
        stmt.query(params![range_start, range_end, backend])
    }
    .map_err(|e| format!("quota_usage_get_stats: query 失败：{e}"))?;

    let mut out = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|e| format!("quota_usage_get_stats: 读取行失败：{e}"))?
    {
        out.push(row_to_usage_record(row)?);
    }
    Ok(out)
}

fn row_to_usage_record(row: &Row<'_>) -> Result<UsageRecord, String> {
    Ok(UsageRecord {
        event_id: row
            .get(0)
            .map_err(|e| format!("quota_usage_get_stats: event_id 失败：{e}"))?,
        task_id: row
            .get(1)
            .map_err(|e| format!("quota_usage_get_stats: task_id 失败：{e}"))?,
        turn_id: row
            .get(2)
            .map_err(|e| format!("quota_usage_get_stats: turn_id 失败：{e}"))?,
        backend: row
            .get(3)
            .map_err(|e| format!("quota_usage_get_stats: backend 失败：{e}"))?,
        session_id: row
            .get(4)
            .map_err(|e| format!("quota_usage_get_stats: session_id 失败：{e}"))?,
        input_tokens: row
            .get(5)
            .map_err(|e| format!("quota_usage_get_stats: input_tokens 失败：{e}"))?,
        output_tokens: row
            .get(6)
            .map_err(|e| format!("quota_usage_get_stats: output_tokens 失败：{e}"))?,
        cache_read_tokens: row
            .get(7)
            .map_err(|e| format!("quota_usage_get_stats: cache_read_tokens 失败：{e}"))?,
        cache_creation_tokens: row
            .get(8)
            .map_err(|e| format!("quota_usage_get_stats: cache_creation_tokens 失败：{e}"))?,
        total_tokens: row
            .get(9)
            .map_err(|e| format!("quota_usage_get_stats: total_tokens 失败：{e}"))?,
        known_cost_usd: row
            .get(10)
            .map_err(|e| format!("quota_usage_get_stats: known_cost_usd 失败：{e}"))?,
        raw_usage_json: row
            .get(11)
            .map_err(|e| format!("quota_usage_get_stats: raw_usage_json 失败：{e}"))?,
        created_at: row
            .get(12)
            .map_err(|e| format!("quota_usage_get_stats: created_at 失败：{e}"))?,
        updated_at: row
            .get(13)
            .map_err(|e| format!("quota_usage_get_stats: updated_at 失败：{e}"))?,
    })
}

pub(crate) fn stats(
    conn: &Connection,
    input: QuotaUsageStatsInput,
    now: i64,
) -> Result<QuotaUsageStats, String> {
    let days = normalize_days(input.days);
    let backend = normalize_backend_filter(input.backend);
    let range_end = day_start(now) + DAY_MS;
    let range_start = range_end - days * DAY_MS;
    let records = load_usage_records(conn, range_start, range_end, &backend)?;

    let mut total = AggregatedUsage::default();
    let mut daily: BTreeMap<i64, AggregatedUsage> = (0..days)
        .map(|offset| (range_start + offset * DAY_MS, AggregatedUsage::default()))
        .collect();
    let mut by_backend: BTreeMap<String, AggregatedUsage> = BTreeMap::new();

    for record in &records {
        total.add_record(record);
        daily
            .entry(day_start(record.created_at))
            .or_default()
            .add_record(record);
        by_backend
            .entry(record.backend.clone())
            .or_default()
            .add_record(record);
    }

    let daily = daily
        .into_iter()
        .map(|(day_start, bucket)| usage_daily_bucket(day_start, bucket))
        .collect();

    let backends = by_backend
        .into_iter()
        .map(|(backend, summary)| usage_backend_summary(backend, summary))
        .collect();

    let mut recent: Vec<_> = records
        .iter()
        .rev()
        .take(RECENT_LIMIT as usize)
        .map(|record| QuotaUsageRecentRecord {
            event_id: record.event_id.clone(),
            task_id: record.task_id.clone(),
            turn_id: record.turn_id.clone(),
            backend: record.backend.clone(),
            session_id: record.session_id.clone(),
            input_tokens: record.input_tokens,
            output_tokens: record.output_tokens,
            cache_read_tokens: record.cache_read_tokens,
            cache_creation_tokens: record.cache_creation_tokens,
            total_tokens: record.total_tokens,
            known_cost_usd: record.known_cost_usd,
            created_at: record.created_at,
        })
        .collect();
    recent.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(QuotaUsageStats {
        days,
        backend,
        range_start,
        range_end,
        cost: QuotaUsageCostCoverage {
            known_cost_usd: total.known_cost(),
            cost_record_count: total.cost_record_count,
            total_record_count: total.record_count,
        },
        totals: total.totals,
        daily,
        backends,
        recent,
    })
}

#[tauri::command]
pub fn quota_usage_get_stats(
    input: Option<QuotaUsageStatsInput>,
    store: State<'_, LiliaStore>,
) -> Result<QuotaUsageStats, String> {
    let conn = store.conn()?;
    stats(
        &conn,
        input.unwrap_or(QuotaUsageStatsInput {
            days: None,
            backend: None,
        }),
        now_millis(),
    )
}

#[tauri::command]
pub fn quota_usage_get_codex_account_status(app: AppHandle) -> CodexAccountQuotaStatus {
    let connection = resolve_connection_for(&app, BACKEND_CODEX);
    if connection.mode != ConnectionMode::CodexAccount {
        return codex_account_quota_unavailable(connection.mode.as_str(), None);
    }
    if let Err(err) = validate_backend_ready_for_send(BACKEND_CODEX) {
        return codex_account_quota_unavailable("codex-account", Some(err));
    }
    let codex_app_server = build_codex_app_server_probe_status();
    let Some(codex_path) = codex_app_server.path else {
        let detail = if codex_app_server.public.issues.is_empty() {
            "未找到满足要求的 Codex CLI，无法读取官方额度。".to_string()
        } else {
            codex_app_server.public.issues.join(" ")
        };
        return codex_account_quota_unavailable("codex-account", Some(detail));
    };
    run_codex_account_quota_utility(&app, codex_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_usage_schema(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE agent_usage_records (
              event_id              TEXT PRIMARY KEY,
              task_id               TEXT NOT NULL,
              turn_id               TEXT,
              backend               TEXT NOT NULL CHECK (backend IN ('claude','codex')),
              session_id            TEXT,
              input_tokens          INTEGER NOT NULL DEFAULT 0,
              output_tokens         INTEGER NOT NULL DEFAULT 0,
              cache_read_tokens     INTEGER NOT NULL DEFAULT 0,
              cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
              total_tokens          INTEGER NOT NULL DEFAULT 0,
              known_cost_usd        REAL,
              raw_usage_json        TEXT NOT NULL,
              created_at            INTEGER NOT NULL,
              updated_at            INTEGER NOT NULL
            );
            CREATE INDEX idx_agent_usage_records_created_at
              ON agent_usage_records(created_at);
            CREATE INDEX idx_agent_usage_records_backend_created
              ON agent_usage_records(backend, created_at);
            "#,
        )
        .unwrap();
    }

    fn timeline_event(
        id: &str,
        backend: &str,
        payload: JsonValue,
        created_at: i64,
    ) -> AgentTimelineEvent {
        AgentTimelineEvent {
            id: id.to_string(),
            task_id: "task-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            backend: backend.to_string(),
            kind: "turn".to_string(),
            status: "success".to_string(),
            title: "turn".to_string(),
            summary: None,
            payload,
            created_at,
            updated_at: created_at,
            turn_seq: 0,
            intra_turn_order: 0,
        }
    }

    #[test]
    fn extracts_claude_snake_case_usage_and_cost() {
        let event = timeline_event(
            "event-1",
            BACKEND_CLAUDE,
            json!({
                "sessionId": "claude-session",
                "totalCostUsd": 0.12,
                "usage": {
                    "input_tokens": 100,
                    "output_tokens": 25,
                    "cache_read_input_tokens": 10,
                    "cache_creation_input_tokens": 5
                }
            }),
            1_000,
        );

        let record = extract_usage_record(&event).unwrap().unwrap();

        assert_eq!(record.input_tokens, 100);
        assert_eq!(record.output_tokens, 25);
        assert_eq!(record.cache_read_tokens, 10);
        assert_eq!(record.cache_creation_tokens, 5);
        assert_eq!(record.total_tokens, 140);
        assert_eq!(record.known_cost_usd, Some(0.12));
        assert_eq!(record.session_id.as_deref(), Some("claude-session"));
    }

    #[test]
    fn extracts_codex_camel_case_usage_without_cost() {
        let event = timeline_event(
            "event-2",
            BACKEND_CODEX,
            json!({
                "sessionId": "thread-1",
                "usage": {
                    "inputTokens": 200,
                    "outputTokens": 50,
                    "cacheReadTokens": 20,
                    "totalTokens": 250
                }
            }),
            1_000,
        );

        let record = extract_usage_record(&event).unwrap().unwrap();

        assert_eq!(record.input_tokens, 200);
        assert_eq!(record.output_tokens, 50);
        assert_eq!(record.cache_read_tokens, 20);
        assert_eq!(record.total_tokens, 270);
        assert_eq!(record.known_cost_usd, None);
    }

    #[test]
    fn upserts_usage_record_for_same_event() {
        let conn = Connection::open_in_memory().unwrap();
        create_usage_schema(&conn);

        let first = timeline_event(
            "event-1",
            BACKEND_CODEX,
            json!({ "usage": { "inputTokens": 10, "outputTokens": 5 } }),
            1_000,
        );
        record_from_timeline_event(&conn, &first).unwrap();
        let updated = timeline_event(
            "event-1",
            BACKEND_CODEX,
            json!({ "usage": { "inputTokens": 20, "outputTokens": 7, "costUsd": 0.03 } }),
            2_000,
        );
        record_from_timeline_event(&conn, &updated).unwrap();

        let row: (i64, i64, Option<f64>, i64) = conn
            .query_row(
                "SELECT input_tokens, output_tokens, known_cost_usd, created_at FROM agent_usage_records WHERE event_id = 'event-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(row, (20, 7, Some(0.03), 2_000));
    }

    #[test]
    fn stats_aggregate_range_backend_filter_and_cost_coverage() {
        let conn = Connection::open_in_memory().unwrap();
        create_usage_schema(&conn);
        let day = DAY_MS;
        for event in [
            timeline_event(
                "claude-1",
                BACKEND_CLAUDE,
                json!({ "totalCostUsd": 0.1, "usage": { "input_tokens": 100, "output_tokens": 10 } }),
                day * 10 + 1,
            ),
            timeline_event(
                "codex-1",
                BACKEND_CODEX,
                json!({ "usage": { "inputTokens": 50, "outputTokens": 20 } }),
                day * 11 + 1,
            ),
            timeline_event(
                "old",
                BACKEND_CLAUDE,
                json!({ "totalCostUsd": 0.2, "usage": { "input_tokens": 999 } }),
                day,
            ),
        ] {
            record_from_timeline_event(&conn, &event).unwrap();
        }

        let all = stats(
            &conn,
            QuotaUsageStatsInput {
                days: Some(7),
                backend: None,
            },
            day * 12 + 5,
        )
        .unwrap();

        assert_eq!(all.totals.total_tokens, 180);
        assert_eq!(all.cost.known_cost_usd, Some(0.1));
        assert_eq!(all.cost.cost_record_count, 1);
        assert_eq!(all.cost.total_record_count, 2);
        assert_eq!(all.daily.len(), 7);
        assert_eq!(all.backends.len(), 2);
        assert_eq!(all.recent.len(), 2);

        let codex = stats(
            &conn,
            QuotaUsageStatsInput {
                days: Some(7),
                backend: Some(BACKEND_CODEX.to_string()),
            },
            day * 12 + 5,
        )
        .unwrap();
        assert_eq!(codex.totals.total_tokens, 70);
        assert_eq!(codex.cost.known_cost_usd, None);
        assert_eq!(codex.backends.len(), 1);
        assert_eq!(codex.backends[0].backend, BACKEND_CODEX);
    }

    #[test]
    fn stats_empty_range_returns_full_daily_buckets() {
        let conn = Connection::open_in_memory().unwrap();
        create_usage_schema(&conn);

        let result = stats(
            &conn,
            QuotaUsageStatsInput {
                days: Some(30),
                backend: Some("all".to_string()),
            },
            DAY_MS * 30,
        )
        .unwrap();

        assert_eq!(result.days, 30);
        assert_eq!(result.totals.total_tokens, 0);
        assert_eq!(result.cost.total_record_count, 0);
        assert_eq!(result.daily.len(), 30);
        assert!(result.backends.is_empty());
        assert!(result.recent.is_empty());
    }
}
