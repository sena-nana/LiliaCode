use std::env;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use tauri::AppHandle;

use crate::{BACKEND_CLAUDE, BACKEND_CODEX};

use super::config::{
    backend_api_key_env, backend_direct_url, load_cc_switch_config, load_provider_config,
    load_router_mode, provider_key_for_backend, CC_SWITCH_PLACEHOLDER_KEY, ROUTER_DIRECT,
};
use super::types::{BackendConnectionPlan, BackendEnvStatus, CCSwitchStatus, ConnectionMode};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_host_port_uses_scheme_defaults_and_existing_port() {
        assert_eq!(
            parse_host_port("127.0.0.1:15721").as_deref(),
            Some("127.0.0.1:15721")
        );
        assert_eq!(
            parse_host_port("http://localhost/path").as_deref(),
            Some("localhost:80")
        );
        assert_eq!(
            parse_host_port("https://api.example.com/v1").as_deref(),
            Some("api.example.com:443")
        );
        assert_eq!(parse_host_port("").as_deref(), None);
    }
}
