use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use mutsuki_agent_bundle::NativeCodingBackends;
use mutsuki_agent_contracts::{
    AgentError, BrowserNavigateRequest, ProcessExecRequest, ProcessExecResult,
};
use mutsuki_agent_plugin_computer_use::{
    BrowserGateway, ProcessGateway, WorkspaceFilesystemBackend,
};
use mutsuki_agent_plugin_git::CliGitBackend;
use mutsuki_agent_plugin_lsp::StdioLspProcessFactory;
use mutsuki_agent_plugin_mcp::{CompositeMcpTransportFactory, ReqwestMcpHttpClient};
use reqwest::blocking::Client;

struct ActiveProcess {
    child: Mutex<Child>,
    cancelled: AtomicBool,
}

/// Local Host process capability used by the shared Computer Use service.
///
/// AgentKit owns the approval plan and handle identity; this backend owns the
/// OS process and makes that handle cancellable.
#[derive(Default)]
pub(crate) struct HostProcessBackend {
    active: Mutex<BTreeMap<String, Arc<ActiveProcess>>>,
}

impl ProcessGateway for HostProcessBackend {
    fn exec(
        &self,
        handle_id: &str,
        request: &ProcessExecRequest,
    ) -> Result<ProcessExecResult, AgentError> {
        let root = std::fs::canonicalize(&request.workspace.root)
            .map_err(|error| AgentError::new("lilia.host.workspace", error.to_string()))?;
        if !root.is_dir() {
            return Err(AgentError::invalid_input(
                "process workspace root must be a directory",
            ));
        }

        let mut command = Command::new(&request.command);
        command
            .args(&request.args)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| AgentError::new("lilia.host.process.spawn", error.to_string()))?;

        if let Some(input) = request.stdin.as_deref() {
            let mut stdin = child.stdin.take().ok_or_else(|| {
                AgentError::new("lilia.host.process.stdin", "process stdin is unavailable")
            })?;
            stdin
                .write_all(input.as_bytes())
                .map_err(|error| AgentError::new("lilia.host.process.stdin", error.to_string()))?;
        }
        drop(child.stdin.take());

        let stdout = child.stdout.take().ok_or_else(|| {
            AgentError::new("lilia.host.process.stdout", "process stdout is unavailable")
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            AgentError::new("lilia.host.process.stderr", "process stderr is unavailable")
        })?;
        let stdout_reader = thread::spawn(move || read_stream(stdout));
        let stderr_reader = thread::spawn(move || read_stream(stderr));

        let active = Arc::new(ActiveProcess {
            child: Mutex::new(child),
            cancelled: AtomicBool::new(false),
        });
        self.active
            .lock()
            .expect("host process registry mutex")
            .insert(handle_id.to_string(), Arc::clone(&active));

        let timeout = Duration::from_millis(request.limits.timeout_ms.max(1));
        let started = Instant::now();
        let status = loop {
            if let Some(status) = active
                .child
                .lock()
                .expect("host child mutex")
                .try_wait()
                .map_err(|error| AgentError::new("lilia.host.process.wait", error.to_string()))?
            {
                break status;
            }
            if started.elapsed() >= timeout {
                active.cancelled.store(true, Ordering::Release);
                let mut child = active.child.lock().expect("host child mutex");
                let _ = child.kill();
                break child.wait().map_err(|error| {
                    AgentError::new("lilia.host.process.wait", error.to_string())
                })?;
            }
            thread::sleep(Duration::from_millis(5));
        };

        self.active
            .lock()
            .expect("host process registry mutex")
            .remove(handle_id);
        let stdout = join_reader(stdout_reader)?;
        let stderr = join_reader(stderr_reader)?;
        let limit = usize::try_from(request.limits.max_output_bytes)
            .unwrap_or(usize::MAX)
            .max(1);
        let mut combined = stdout;
        if !stderr.is_empty() {
            if !combined.is_empty() {
                combined.push(b'\n');
            }
            combined.extend_from_slice(&stderr);
        }
        let truncated = combined.len() > limit;
        combined.truncate(limit);

        Ok(ProcessExecResult {
            exit_code: status.code().unwrap_or(-1),
            summary: String::from_utf8_lossy(&combined).into_owned(),
            stdout_ref: None,
            stderr_ref: None,
            truncated,
            cancelled: active.cancelled.load(Ordering::Acquire),
        })
    }

    fn cancel(&self, handle_id: &str) -> Result<(), AgentError> {
        let active = self
            .active
            .lock()
            .expect("host process registry mutex")
            .get(handle_id)
            .cloned();
        if let Some(active) = active {
            active.cancelled.store(true, Ordering::Release);
            active
                .child
                .lock()
                .expect("host child mutex")
                .kill()
                .map_err(|error| AgentError::new("lilia.host.process.cancel", error.to_string()))?;
        }
        Ok(())
    }
}

fn read_stream(mut stream: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn join_reader(
    reader: thread::JoinHandle<std::io::Result<Vec<u8>>>,
) -> Result<Vec<u8>, AgentError> {
    reader
        .join()
        .map_err(|_| AgentError::new("lilia.host.process.read", "process reader panicked"))?
        .map_err(|error| AgentError::new("lilia.host.process.read", error.to_string()))
}

pub(crate) struct HostHttpBackend;

impl BrowserGateway for HostHttpBackend {
    fn snapshot(
        &self,
        request: &BrowserNavigateRequest,
    ) -> Result<(String, String, Vec<u8>), AgentError> {
        let cancelled = Arc::new(AtomicBool::new(false));
        self.snapshot_inner(request, &cancelled)
    }
    fn cancel(&self, _handle_id: &str) -> Result<(), AgentError> { Ok(()) }
}

impl HostHttpBackend {
    fn snapshot_inner(&self, request: &BrowserNavigateRequest, cancelled: &AtomicBool) -> Result<(String, String, Vec<u8>), AgentError> {
        let mut current_url = reqwest::Url::parse(&request.url)
            .map_err(|_| AgentError::invalid_input("browser URL is invalid"))?;
        if cancelled.load(Ordering::Acquire) { return Err(AgentError::new("lilia.host.http.cancelled", "browser request cancelled")); }
        let response = (0..=3)
            .find_map(|redirect| {
                let addresses = browser_url_addresses(current_url.as_str())?;
                let host = current_url.host_str()?;
                let client = match Client::builder()
                    .timeout(Duration::from_millis(request.limits.timeout_ms.max(1)))
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .resolve_to_addrs(host, &addresses)
                    .build()
                {
                    Ok(client) => client,
                    Err(error) => return Some(Err(AgentError::new("lilia.host.http.client", error.to_string()))),
                };
                if cancelled.load(Ordering::Acquire) { return Some(Err(AgentError::new("lilia.host.http.cancelled", "browser request cancelled"))); }
                let response = match client.get(current_url.clone()).send() {
                    Ok(response) => response,
                    Err(error) => return Some(Err(AgentError::new("lilia.host.http.request", error.to_string()))),
                };
                if response.status().is_redirection() {
                    if redirect == 3 {
                        return Some(Err(AgentError::new(
                            "lilia.host.http.redirect",
                            "too many browser redirects",
                        )));
                    }
                    let Some(location) = response.headers().get(reqwest::header::LOCATION) else {
                        return Some(Err(AgentError::new("lilia.host.http.redirect", "redirect is missing Location")));
                    };
                    let Ok(location) = location.to_str() else {
                        return Some(Err(AgentError::new("lilia.host.http.redirect", "redirect Location is invalid")));
                    };
                    let Ok(next_url) = current_url.join(location) else {
                        return Some(Err(AgentError::new("lilia.host.http.redirect", "redirect Location is invalid")));
                    };
                    current_url = next_url;
                    None
                } else {
                    Some(response.error_for_status().map_err(|error| {
                        AgentError::new("lilia.host.http.request", error.to_string())
                    }))
                }
            })
            .ok_or_else(|| AgentError::new("lilia.host.http.redirect", "invalid browser redirect"))??;
        let final_url = current_url.to_string();
        let limit = usize::try_from(request.limits.max_output_bytes)
            .unwrap_or(usize::MAX)
            .max(1);
        if response
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err(AgentError::new(
                "lilia.host.http.body",
                "browser response exceeds the output limit",
            ));
        }
        let mut body = Vec::with_capacity(limit.min(64 * 1024));
        if cancelled.load(Ordering::Acquire) { return Err(AgentError::new("lilia.host.http.cancelled", "browser request cancelled")); }
        response
            .take(limit as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|error| AgentError::new("lilia.host.http.body", error.to_string()))?;
        if body.len() > limit {
            return Err(AgentError::new(
                "lilia.host.http.body",
                "browser response exceeds the output limit",
            ));
        }
        let title = html_title(&body).unwrap_or_else(|| final_url.clone());
        Ok((final_url, title, body))
    }

    fn cancel(&self, _handle_id: &str) -> Result<(), AgentError> { Ok(()) }
}

#[cfg(test)]
fn browser_url_allowed(value: &str) -> bool {
    browser_url_addresses(value).is_some_and(|addresses| !addresses.is_empty())
}

fn browser_url_addresses(value: &str) -> Option<Vec<SocketAddr>> {
    let Ok(url) = reqwest::Url::parse(value) else { return None; };
    if !matches!(url.scheme(), "http" | "https") || url.username() != "" || url.password().is_some() {
        return None;
    }
    let Some(host) = url.host_str() else { return None; };
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return None;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return public_ip(ip).then(|| vec![SocketAddr::new(ip, url.port_or_known_default().unwrap_or(443))]);
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let addrs = (host, port).to_socket_addrs().ok()?.collect::<Vec<_>>();
    (!addrs.is_empty() && addrs.iter().all(|addr| public_ip(addr.ip()))).then_some(addrs)
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => !(ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() || ip.is_broadcast() || ip.octets()[0] == 0 || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]))),
        IpAddr::V6(ip) => !(ip.is_loopback() || ip.is_unspecified() || ip.is_unique_local() || ip.is_unicast_link_local()),
    }
}

fn html_title(body: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(body);
    let lower = text.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let content_start = lower[start..].find('>')? + start + 1;
    let end = lower[content_start..].find("</title>")? + content_start;
    let title = text[content_start..end].trim();
    (!title.is_empty()).then(|| title.to_string())
}

pub(crate) fn native_coding_backends() -> NativeCodingBackends {
    NativeCodingBackends {
        git: Arc::new(CliGitBackend::default()),
        filesystem: Arc::new(WorkspaceFilesystemBackend),
        process: Some(Arc::new(HostProcessBackend::default())),
        browser: Some(Arc::new(HostHttpBackend)),
        lsp: Arc::new(StdioLspProcessFactory),
        mcp: Arc::new(CompositeMcpTransportFactory::new(Arc::new(
            ReqwestMcpHttpClient,
        ))),
        code_index_lsp: Arc::new(mutsuki_agent_plugin_code_index::UnavailableLspSignals),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mutsuki_agent_contracts::{AgentWorkspaceRef, ExecutionLimits};

    #[test]
    fn process_backend_executes_in_workspace_and_honors_output_limit() {
        let root = std::env::temp_dir().join("lilia-host-process-backend");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let backend = HostProcessBackend::default();
        #[cfg(windows)]
        let (command, args) = ("cmd".to_string(), vec!["/C".into(), "echo 123456".into()]);
        #[cfg(not(windows))]
        let (command, args) = ("sh".to_string(), vec!["-c".into(), "printf 123456".into()]);
        let result = backend
            .exec(
                "process-test",
                &ProcessExecRequest {
                    workspace: AgentWorkspaceRef {
                        workspace_id: "test".into(),
                        root: root.display().to_string(),
                    },
                    command,
                    args,
                    stdin: None,
                    limits: ExecutionLimits {
                        timeout_ms: 1_000,
                        max_output_bytes: 4,
                        max_concurrency: 1,
                    },
                    allow_network: false,
                },
            )
            .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(&result.summary[..4.min(result.summary.len())], "1234");
        assert!(result.truncated);
        assert!(!result.cancelled);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn title_is_extracted_from_html() {
        assert_eq!(
            html_title(b"<html><TITLE>Workspace</TITLE></html>").as_deref(),
            Some("Workspace")
        );
    }

    #[test]
    fn browser_url_policy_rejects_local_and_credential_bearing_targets() {
        assert!(!browser_url_allowed("http://127.0.0.1/") );
        assert!(!browser_url_allowed("http://localhost/"));
        assert!(!browser_url_allowed("http://user:pass@example.com/"));
        assert!(!browser_url_allowed("file:///etc/passwd"));
    }
}
