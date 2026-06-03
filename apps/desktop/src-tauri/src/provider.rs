use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::AppHandle;

use crate::chat::state::normalize_backend;
use crate::settings_store::{load_store_value, save_store_value};
use crate::{BACKEND_CLAUDE, BACKEND_CODEX, MIN_CODEX_APP_SERVER_VERSION};

const CC_SWITCH_DEFAULT_URL: &str = "http://127.0.0.1:15721";
const CC_SWITCH_PLACEHOLDER_KEY: &str = "sk-cc-switch-proxy";
const PROVIDER_ACTIVE_BACKEND_KEY: &str = "provider.activeBackend";
const PROVIDER_KEY_CLAUDE: &str = "provider.claude";
const PROVIDER_KEY_CODEX: &str = "provider.codex";
const CC_SWITCH_KEY: &str = "cc-switch.config";
const ROUTER_KEY_CLAUDE: &str = "router.claude";
const ROUTER_KEY_CODEX: &str = "router.codex";
const ASSISTANT_AI_KEY: &str = "assistant-ai.config";
const AGENT_INTERACTION_KEY: &str = "agent-interaction.config";
const ROUTER_CC_SWITCH: &str = "cc-switch";
const ROUTER_DIRECT: &str = "direct";
static CODEX_APP_SERVER_PROBE_CACHE: OnceLock<Mutex<Option<CodexAppServerProbeStatus>>> =
    OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentInteractionSettings {
    #[serde(default)]
    pub(crate) non_interrupt_mode: bool,
    #[serde(default)]
    pub(crate) debug: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderConfig {
    pub(crate) backend: String,
    pub(crate) base_url: Option<String>,
    pub(crate) api_key: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CCSwitchConfig {
    pub(crate) base_url: Option<String>,
}

impl Default for CCSwitchConfig {
    fn default() -> Self {
        CCSwitchConfig {
            base_url: Some(CC_SWITCH_DEFAULT_URL.to_string()),
        }
    }
}

/// 辅助模型（Assistant AI）配置。独立于 ProviderConfig，**不参与 Agent 主循环**，
/// 仅供周边系统（Memory 助手、Tool Call 后处理、摘要等）消费。
/// 三件套齐全才算启用；任一缺失消费方应 short-circuit 跳过。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssistantAIConfig {
    pub(crate) base_url: Option<String>,
    pub(crate) api_key: Option<String>,
    pub(crate) model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssistantAITestResult {
    pub(crate) ok: bool,
    pub(crate) error: Option<String>,
    /// 端点不支持 /models 时为 None，UI 据此降级提示。
    pub(crate) models: Option<Vec<String>>,
    /// 配置里的 model 是否出现在 models 列表里。models 为 None 时也为 None。
    pub(crate) model_matched: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackendEnvStatus {
    pub(crate) backend: String,
    pub(crate) has_api_key: bool,
    /// "cc-switch" | "custom" | "direct" | "unconfigured"
    pub(crate) connection_mode: String,
    pub(crate) effective_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CCSwitchStatus {
    pub(crate) reachable: bool,
    pub(crate) base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexAppServerStatus {
    pub(crate) version: Option<String>,
    pub(crate) available: bool,
    pub(crate) supports_required_protocol: bool,
    pub(crate) issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnvStatusReport {
    pub(crate) node_available: bool,
    /// codex CLI 是否能在 PATH 找到。
    pub(crate) codex_cli_available: bool,
    /// Codex app-server / dynamic tool / AskUser 所需能力检查。
    pub(crate) codex_app_server: CodexAppServerStatus,
    pub(crate) cc_switch: CCSwitchStatus,
    /// 每个 backend 当前生效的路由模式（"cc-switch" | "direct"）。
    pub(crate) router_modes: HashMap<String, String>,
    pub(crate) backends: HashMap<String, BackendEnvStatus>,
}

// ---------- 连接解析 ----------

/// 与前端 ConnectionMode 字符串对齐的四档枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectionMode {
    CcSwitch,
    CustomBaseUrl,
    Direct,
    Unconfigured,
}

impl ConnectionMode {
    fn as_str(self) -> &'static str {
        match self {
            ConnectionMode::CcSwitch => "cc-switch",
            ConnectionMode::CustomBaseUrl => "custom",
            ConnectionMode::Direct => "direct",
            ConnectionMode::Unconfigured => "unconfigured",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BackendConnectionPlan {
    pub(crate) mode: ConnectionMode,
    pub(crate) base_url: Option<String>,
    pub(crate) api_key: Option<String>,
}

fn provider_key_for_backend(backend: &str) -> &'static str {
    match backend {
        BACKEND_CODEX => PROVIDER_KEY_CODEX,
        _ => PROVIDER_KEY_CLAUDE,
    }
}

fn known_provider_key_for_backend(backend: &str) -> Result<&'static str, String> {
    match backend {
        BACKEND_CODEX => Ok(PROVIDER_KEY_CODEX),
        BACKEND_CLAUDE => Ok(PROVIDER_KEY_CLAUDE),
        other => Err(format!("未知 backend: {other}")),
    }
}

fn router_key_for_backend(backend: &str) -> Result<&'static str, String> {
    match backend {
        BACKEND_CODEX => Ok(ROUTER_KEY_CODEX),
        BACKEND_CLAUDE => Ok(ROUTER_KEY_CLAUDE),
        other => Err(format!("未知 backend: {other}")),
    }
}

fn backend_api_key_env(backend: &str) -> &'static str {
    match backend {
        BACKEND_CODEX => "OPENAI_API_KEY",
        _ => "ANTHROPIC_API_KEY",
    }
}

fn backend_direct_url(backend: &str) -> &'static str {
    match backend {
        BACKEND_CODEX => "https://api.openai.com/v1",
        _ => "https://api.anthropic.com",
    }
}

/// 探测一个 base URL 是否可拨通。短超时——拨不通就当它不存在，不阻塞主流程。
pub(crate) fn url_reachable(url: Option<&str>) -> bool {
    let Some(url) = url else { return false };
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return false;
    }
    let Some(host_port) = parse_host_port(trimmed) else {
        return false;
    };
    let Ok(addrs) = host_port.to_socket_addrs() else {
        return false;
    };
    addrs
        .into_iter()
        .any(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok())
}

/// 从 `http(s)://host:port[/...]` 抽出 `host:port`；缺端口时按协议给默认。
pub(crate) fn parse_host_port(url: &str) -> Option<String> {
    let (scheme, rest) = if let Some(r) = url.strip_prefix("http://") {
        ("http", r)
    } else if let Some(r) = url.strip_prefix("https://") {
        ("https", r)
    } else {
        ("http", url)
    };
    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty() {
        return None;
    }
    if authority.contains(':') {
        Some(authority.to_string())
    } else {
        let default_port = if scheme == "https" { 443 } else { 80 };
        Some(format!("{authority}:{default_port}"))
    }
}

/// 从 tauri_plugin_store 读 ProviderConfig。
pub(crate) fn load_provider_config(app: &AppHandle, key: &str) -> Option<ProviderConfig> {
    load_store_value(app, key)
}

pub(crate) fn load_active_backend(app: &AppHandle) -> String {
    load_store_value::<String>(app, PROVIDER_ACTIVE_BACKEND_KEY)
        .map(|s| normalize_backend(&s).to_string())
        .unwrap_or_else(|| BACKEND_CLAUDE.to_string())
}

/// 读 CC-Switch 配置；读不到回 Default。
pub(crate) fn load_cc_switch_config(app: &AppHandle) -> CCSwitchConfig {
    load_store_value(app, CC_SWITCH_KEY).unwrap_or_default()
}

/// 读辅助模型配置；读不到回 Default（三件套全 None）。
pub(crate) fn load_assistant_ai_config(app: &AppHandle) -> AssistantAIConfig {
    load_store_value(app, ASSISTANT_AI_KEY).unwrap_or_default()
}

pub(crate) fn load_agent_interaction_settings(app: &AppHandle) -> AgentInteractionSettings {
    load_store_value(app, AGENT_INTERACTION_KEY).unwrap_or_default()
}

/// 读某个 backend 的路由模式；未设置返回默认 "cc-switch"。
pub(crate) fn load_router_mode(app: &AppHandle, backend: &str) -> String {
    let key = router_key_for_backend(normalize_backend(backend)).unwrap_or(ROUTER_KEY_CLAUDE);
    load_store_value::<String>(app, key)
        .filter(|m| matches!(m.as_str(), ROUTER_CC_SWITCH | ROUTER_DIRECT))
        .unwrap_or_else(|| ROUTER_CC_SWITCH.to_string())
}

/// CC-Switch 路由：检查共用代理 URL 是否非空 + 可达。
pub(crate) fn try_cc_switch_for_backend(app: &AppHandle) -> Option<BackendConnectionPlan> {
    let cfg = load_cc_switch_config(app);
    let url = cfg.base_url.filter(|s| !s.is_empty())?;
    if !url_reachable(Some(&url)) {
        return None;
    }
    Some(BackendConnectionPlan {
        mode: ConnectionMode::CcSwitch,
        base_url: Some(url),
        api_key: Some(CC_SWITCH_PLACEHOLDER_KEY.to_string()),
    })
}

/// direct 路由：用 store 里的 ProviderConfig；apiKey/baseUrl 都空则 unconfigured。
pub(crate) fn try_direct_for_backend(
    app: &AppHandle,
    backend: &'static str,
) -> BackendConnectionPlan {
    let cfg = load_provider_config(app, provider_key_for_backend(backend)).unwrap_or_default();
    let has_key = cfg.api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false);
    let has_url = cfg
        .base_url
        .as_ref()
        .map(|u| !u.is_empty())
        .unwrap_or(false);
    if !has_key && !has_url {
        return BackendConnectionPlan {
            mode: ConnectionMode::Unconfigured,
            base_url: None,
            api_key: None,
        };
    }
    let mode = if has_url {
        ConnectionMode::CustomBaseUrl
    } else {
        ConnectionMode::Direct
    };
    BackendConnectionPlan {
        mode,
        base_url: cfg.base_url.filter(|s| !s.is_empty()),
        api_key: cfg.api_key.filter(|s| !s.is_empty()),
    }
}

/// 入口：按 per-backend 路由模式分发。选了哪个就只走哪个，失败即 unconfigured。
pub(crate) fn resolve_connection_for(app: &AppHandle, backend_str: &str) -> BackendConnectionPlan {
    let backend: &'static str = if backend_str == BACKEND_CODEX {
        BACKEND_CODEX
    } else {
        BACKEND_CLAUDE
    };
    let mode = load_router_mode(app, backend);
    match mode.as_str() {
        ROUTER_DIRECT => try_direct_for_backend(app, backend),
        _ => try_cc_switch_for_backend(app).unwrap_or(BackendConnectionPlan {
            mode: ConnectionMode::Unconfigured,
            base_url: None,
            api_key: None,
        }),
    }
}
pub(crate) fn build_backend_env_status(app: &AppHandle, backend: &str) -> BackendEnvStatus {
    let plan = resolve_connection_for(app, backend);

    // has_api_key 综合 plan / env / direct 配置三处；让 UI 区分「没配过」和「配了但当前路由没用上」。
    let key_env = backend_api_key_env(backend);
    let has_api_key = plan
        .api_key
        .as_ref()
        .map(|k| !k.is_empty())
        .unwrap_or(false)
        || env::var(key_env).map(|v| !v.is_empty()).unwrap_or(false)
        || load_provider_config(app, provider_key_for_backend(backend))
            .and_then(|c| c.api_key.filter(|s| !s.is_empty()))
            .is_some();

    let effective_url = match plan.mode {
        ConnectionMode::CcSwitch => plan.base_url.clone(),
        ConnectionMode::CustomBaseUrl => plan.base_url.clone(),
        ConnectionMode::Direct => Some(backend_direct_url(backend).to_string()),
        ConnectionMode::Unconfigured => None,
    };

    BackendEnvStatus {
        backend: backend.to_string(),
        has_api_key,
        connection_mode: plan.mode.as_str().to_string(),
        effective_url,
    }
}

pub(crate) fn build_cc_switch_status(app: &AppHandle) -> CCSwitchStatus {
    let cfg = load_cc_switch_config(app);
    let url = cfg.base_url.filter(|s| !s.is_empty());
    CCSwitchStatus {
        reachable: url_reachable(url.as_deref()),
        base_url: url,
    }
}

pub(crate) fn cli_available(name: &str) -> bool {
    let candidates: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    for ext in candidates {
        let candidate = format!("{name}{ext}");
        let ok = Command::new(&candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexCliProbe {
    pub(crate) program: String,
    pub(crate) version_output: String,
    pub(crate) app_server_help: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CodexAppServerProbeStatus {
    pub(crate) public: CodexAppServerStatus,
    pub(crate) path: Option<String>,
}

pub(crate) fn codex_candidate_filenames() -> &'static [&'static str] {
    if cfg!(windows) {
        &["codex.cmd", "codex.exe", "codex.bat", "codex"]
    } else {
        &["codex"]
    }
}

pub(crate) fn windows_native_codex_paths() -> Vec<String> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let Some(local_app_data) = env::var_os("LOCALAPPDATA") else {
        return Vec::new();
    };
    let bin = PathBuf::from(local_app_data)
        .join("OpenAI")
        .join("Codex")
        .join("bin");
    let mut paths = Vec::new();
    let root_codex = bin.join("codex.exe");
    if root_codex.exists() {
        paths.push(root_codex.to_string_lossy().to_string());
    }
    if let Ok(entries) = fs::read_dir(&bin) {
        for entry in entries.flatten() {
            let path = entry.path().join("codex.exe");
            if path.exists() {
                paths.push(path.to_string_lossy().to_string());
            }
        }
    }
    paths
}

pub(crate) fn normalize_candidate_key(path: &str) -> String {
    if cfg!(windows) {
        path.to_ascii_lowercase()
    } else {
        path.to_string()
    }
}

pub(crate) fn push_unique_candidate(
    candidates: &mut Vec<String>,
    seen: &mut HashSet<String>,
    path: String,
) {
    if path.trim().is_empty() {
        return;
    }
    let key = normalize_candidate_key(path.trim());
    if seen.insert(key) {
        candidates.push(path.trim().to_string());
    }
}

pub(crate) fn is_windows_apps_candidate(path: &str) -> bool {
    cfg!(windows) && path.to_ascii_lowercase().contains("\\windowsapps\\")
}

pub(crate) fn where_codex_candidates() -> Vec<String> {
    let output = Command::new(if cfg!(windows) { "where.exe" } else { "which" })
        .arg("codex")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub(crate) fn path_codex_candidates() -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(path_var) = env::var_os("PATH") {
        for dir in env::split_paths(&path_var) {
            for filename in codex_candidate_filenames() {
                let candidate = dir.join(filename);
                if candidate.exists() {
                    paths.push(candidate.to_string_lossy().to_string());
                }
            }
        }
    }
    paths
}

pub(crate) fn codex_cli_candidate_paths() -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let path_candidates = path_codex_candidates();
    let where_candidates = where_codex_candidates();
    for path in &path_candidates {
        if !is_windows_apps_candidate(path) {
            push_unique_candidate(&mut candidates, &mut seen, path.clone());
        }
    }
    for path in windows_native_codex_paths() {
        push_unique_candidate(&mut candidates, &mut seen, path);
    }
    for path in &where_candidates {
        if !is_windows_apps_candidate(path) {
            push_unique_candidate(&mut candidates, &mut seen, path.clone());
        }
    }
    for candidate in codex_candidate_filenames() {
        push_unique_candidate(&mut candidates, &mut seen, (*candidate).to_string());
    }
    for path in path_candidates.into_iter().chain(where_candidates) {
        if is_windows_apps_candidate(&path) {
            push_unique_candidate(&mut candidates, &mut seen, path);
        }
    }
    candidates
}

pub(crate) fn command_output_result(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !err.is_empty() { err } else { out };
        return Err(if detail.is_empty() {
            format!("command exited with {}", output.status)
        } else {
            detail
        });
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !text.is_empty() {
        return Ok(text);
    }
    let text = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if text.is_empty() {
        Err("command produced no output".to_string())
    } else {
        Ok(text)
    }
}

pub(crate) fn resolve_codex_cli_probe_with<F>(
    candidates: &[String],
    mut command_output: F,
) -> Option<CodexCliProbe>
where
    F: FnMut(&str, &[&str]) -> Result<String, String>,
{
    for candidate in candidates {
        let version = match command_output(candidate, &["--version"]) {
            Ok(output) => output,
            Err(_) => continue,
        };
        let parsed_version = parse_codex_cli_version(&version);
        let help = match command_output(candidate, &["app-server", "--help"]) {
            Ok(output) => output,
            Err(_) => continue,
        };
        let app_server_available = help.contains("codex app-server") || help.contains("Usage:");
        let version_supported = parsed_version
            .map(|version| codex_version_at_least(version, MIN_CODEX_APP_SERVER_VERSION))
            .unwrap_or(false);
        if app_server_available && version_supported {
            return Some(CodexCliProbe {
                program: candidate.clone(),
                version_output: version,
                app_server_help: help,
            });
        }
    }
    None
}

pub(crate) fn parse_codex_cli_version(output: &str) -> Option<(u32, u32, u32)> {
    let version = output
        .split_whitespace()
        .find(|part| part.chars().next().is_some_and(|ch| ch.is_ascii_digit()))?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u32>().ok()?;
    let patch_text = parts.next().unwrap_or("0");
    let patch_digits: String = patch_text
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    let patch = patch_digits.parse::<u32>().ok()?;
    Some((major, minor, patch))
}

pub(crate) fn codex_version_at_least(version: (u32, u32, u32), minimum: (u32, u32, u32)) -> bool {
    version >= minimum
}

pub(crate) fn build_codex_app_server_probe_status_with<F>(
    candidates: &[String],
    mut command_output: F,
) -> CodexAppServerProbeStatus
where
    F: FnMut(&str, &[&str]) -> Result<String, String>,
{
    let codex_cli = resolve_codex_cli_probe_with(candidates, &mut command_output);
    let mut issues = Vec::new();
    let Some(codex_cli) = codex_cli else {
        if candidates.is_empty() {
            issues.push("未找到 codex CLI。请先安装 Codex CLI 后重新检测。".to_string());
        } else {
            issues.push("Codex app-server 环境不满足。".to_string());
        }
        return CodexAppServerProbeStatus {
            public: CodexAppServerStatus {
                version: None,
                available: false,
                supports_required_protocol: false,
                issues,
            },
            path: None,
        };
    };

    let parsed_version = parse_codex_cli_version(&codex_cli.version_output);
    if parsed_version.is_none() {
        issues.push("无法读取 codex CLI 版本。".to_string());
    }

    let available = codex_cli.app_server_help.contains("codex app-server")
        || codex_cli.app_server_help.contains("Usage:");
    if !available {
        issues.push("当前 codex CLI 不支持 app-server 子命令。".to_string());
    }

    let version_supported = parsed_version
        .map(|version| codex_version_at_least(version, MIN_CODEX_APP_SERVER_VERSION))
        .unwrap_or(false);
    if !version_supported {
        issues.push("当前 codex CLI 版本过低，需要 0.128.0 或更新版本以支持 Lilia 的流式事件、工具审批和 AskUser。".to_string());
    }

    let path = Some(codex_cli.program);
    CodexAppServerProbeStatus {
        public: CodexAppServerStatus {
            version: Some(codex_cli.version_output),
            available,
            supports_required_protocol: available && version_supported,
            issues,
        },
        path,
    }
}

pub(crate) fn build_codex_app_server_probe_status() -> CodexAppServerProbeStatus {
    build_codex_app_server_probe_status_with(&codex_cli_candidate_paths(), command_output_result)
}

pub(crate) fn build_codex_app_server_probe_status_cached(
    force_refresh: bool,
) -> CodexAppServerProbeStatus {
    let cache = CODEX_APP_SERVER_PROBE_CACHE.get_or_init(|| Mutex::new(None));
    if !force_refresh {
        if let Ok(guard) = cache.lock() {
            if let Some(status) = guard.clone() {
                return status;
            }
        }
    }
    let status = build_codex_app_server_probe_status();
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(status.clone());
    }
    status
}

pub(crate) fn codex_send_block_reason(status: &CodexAppServerStatus) -> Option<String> {
    if status.supports_required_protocol {
        return None;
    }

    let detail = if status.issues.is_empty() {
        "Codex app-server 环境不满足。".to_string()
    } else {
        status.issues.join(" ")
    };
    Some(format!(
        "{detail} 请升级 Codex CLI 到 0.128.0 或更新版本；并确认 CC-Switch 当前选中的上游 provider 支持 OpenAI Responses API 与 Codex 模型白名单。"
    ))
}

pub(crate) fn validate_backend_ready_for_send(active_backend: &str) -> Result<(), String> {
    if active_backend != BACKEND_CODEX {
        return Ok(());
    }
    let status = build_codex_app_server_probe_status();
    if let Some(reason) = codex_send_block_reason(&status.public) {
        return Err(reason);
    }
    Ok(())
}

#[tauri::command]
pub fn chat_check_env(app: AppHandle, force_refresh: Option<bool>) -> EnvStatusReport {
    let node_available = cli_available("node");
    let codex_app_server =
        build_codex_app_server_probe_status_cached(force_refresh.unwrap_or(false));
    let codex_cli_available = codex_app_server.path.is_some();

    let mut backends = HashMap::new();
    backends.insert(
        BACKEND_CLAUDE.to_string(),
        build_backend_env_status(&app, BACKEND_CLAUDE),
    );
    backends.insert(
        BACKEND_CODEX.to_string(),
        build_backend_env_status(&app, BACKEND_CODEX),
    );

    let mut router_modes = HashMap::new();
    router_modes.insert(
        BACKEND_CLAUDE.to_string(),
        load_router_mode(&app, BACKEND_CLAUDE),
    );
    router_modes.insert(
        BACKEND_CODEX.to_string(),
        load_router_mode(&app, BACKEND_CODEX),
    );

    EnvStatusReport {
        node_available,
        codex_cli_available,
        codex_app_server: codex_app_server.public,
        cc_switch: build_cc_switch_status(&app),
        router_modes,
        backends,
    }
}

#[tauri::command]
pub fn provider_get_config(app: AppHandle, backend: String) -> ProviderConfig {
    load_provider_config(&app, provider_key_for_backend(&backend)).unwrap_or_else(|| {
        ProviderConfig {
            backend: backend.clone(),
            base_url: None,
            api_key: None,
        }
    })
}

#[tauri::command]
pub fn provider_set_config(app: AppHandle, config: ProviderConfig) -> Result<(), String> {
    let key = known_provider_key_for_backend(&config.backend)?;
    save_store_value(&app, key, &config)
}

#[tauri::command]
pub fn provider_get_active_backend(app: AppHandle) -> String {
    load_active_backend(&app)
}

#[tauri::command]
pub fn provider_set_active_backend(app: AppHandle, backend: String) -> Result<(), String> {
    match backend.as_str() {
        BACKEND_CLAUDE | BACKEND_CODEX => {
            save_store_value(&app, PROVIDER_ACTIVE_BACKEND_KEY, &backend)
        }
        other => Err(format!("未知 backend: {other}")),
    }
}

#[tauri::command]
pub fn cc_switch_get_config(app: AppHandle) -> CCSwitchConfig {
    load_cc_switch_config(&app)
}

#[tauri::command]
pub fn cc_switch_set_config(app: AppHandle, config: CCSwitchConfig) -> Result<(), String> {
    save_store_value(&app, CC_SWITCH_KEY, &config)
}

#[tauri::command]
pub fn assistant_ai_get_config(app: AppHandle) -> AssistantAIConfig {
    load_assistant_ai_config(&app)
}

#[tauri::command]
pub fn assistant_ai_set_config(app: AppHandle, config: AssistantAIConfig) -> Result<(), String> {
    save_store_value(&app, ASSISTANT_AI_KEY, &config)
}

#[tauri::command]
pub fn agent_interaction_get_settings(app: AppHandle) -> AgentInteractionSettings {
    load_agent_interaction_settings(&app)
}

#[tauri::command]
pub fn agent_interaction_set_settings(
    app: AppHandle,
    settings: AgentInteractionSettings,
) -> Result<(), String> {
    save_store_value(&app, AGENT_INTERACTION_KEY, &settings)
}

/// 连通性 ping：GET {baseUrl}/models，3 秒超时。
/// 不消耗 token，能同时验证 baseUrl 可达、apiKey 被接受、配置的 model 是否在列表里。
#[tauri::command]
pub fn assistant_ai_test_connection(config: AssistantAIConfig) -> AssistantAITestResult {
    let base = config
        .base_url
        .as_deref()
        .unwrap_or("")
        .trim()
        .trim_end_matches('/');
    let key = config.api_key.as_deref().unwrap_or("").trim();
    let model = config.model.as_deref().unwrap_or("").trim();
    if base.is_empty() || key.is_empty() || model.is_empty() {
        return AssistantAITestResult {
            ok: false,
            error: Some("baseUrl / apiKey / model 必须全部填写".into()),
            models: None,
            model_matched: None,
        };
    }
    let url = format!("{base}/models");
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return AssistantAITestResult {
                ok: false,
                error: Some(format!("HTTP 客户端构造失败：{e}")),
                models: None,
                model_matched: None,
            }
        }
    };
    match client.get(&url).bearer_auth(key).send() {
        Ok(resp) if resp.status().is_success() => {
            let parsed: Option<Vec<String>> = resp
                .json::<JsonValue>()
                .ok()
                .and_then(|v| v.get("data").cloned())
                .and_then(|d| d.as_array().cloned())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(String::from))
                        .collect()
                });
            let matched = parsed.as_ref().map(|list| list.iter().any(|m| m == model));
            AssistantAITestResult {
                ok: true,
                error: None,
                models: parsed,
                model_matched: matched,
            }
        }
        Ok(resp) => AssistantAITestResult {
            ok: false,
            error: Some(format!("HTTP {} from {url}", resp.status())),
            models: None,
            model_matched: None,
        },
        Err(e) => AssistantAITestResult {
            ok: false,
            error: Some(format!("请求失败：{e}")),
            models: None,
            model_matched: None,
        },
    }
}

#[tauri::command]
pub fn router_get_mode(app: AppHandle, backend: String) -> String {
    load_router_mode(&app, &backend)
}

#[tauri::command]
pub fn router_set_mode(app: AppHandle, backend: String, mode: String) -> Result<(), String> {
    if !matches!(mode.as_str(), ROUTER_CC_SWITCH | ROUTER_DIRECT) {
        return Err(format!("未知路由模式: {mode}"));
    }
    let key = router_key_for_backend(&backend)?;
    save_store_value(&app, key, &JsonValue::String(mode))
}
