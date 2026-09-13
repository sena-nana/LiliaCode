use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::{repo_root, run as run_command, Result, XtaskError};

const READY_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(15);
const DISCARD_MODEL_ENDPOINT: &str = "http://127.0.0.1:9/v1/chat/completions";

#[path = "agent_debug_session.rs"]
mod session_restart;

#[path = "agent_debug_automation.rs"]
mod automation_replay;

#[path = "agent_debug_knowledge.rs"]
mod knowledge_replay;

#[path = "agent_debug_memory_popup.rs"]
mod memory_popup_replay;

#[path = "agent_debug_quota.rs"]
mod quota_replay;

#[path = "agent_debug_extensions_execution.rs"]
mod extensions_execution;

#[path = "agent_debug_source.rs"]
mod source;

mod automation_view;
mod browser_agent;
mod settings_view;
mod task_view;

fn binary_fingerprint(path: &Path) -> Result<Value> {
    use sha2::{Digest, Sha256};
    let mut file = fs::File::open(path)
        .map_err(|e| XtaskError::io("debug_binary_missing", "open desktop artifact", e))?;
    let metadata = file.metadata().map_err(|e| {
        XtaskError::io("debug_binary_metadata", "read desktop artifact metadata", e)
    })?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let size = file
            .read(&mut buffer)
            .map_err(|e| XtaskError::io("debug_binary_hash", "hash desktop artifact", e))?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(serde_json::json!({
        "path": path,
        "sha256": hex::encode(hash.finalize()),
        "sizeBytes": metadata.len(),
        "modifiedUnixMs": metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis()),
    }))
}

pub(crate) fn verify_same_binary(first_run: &Path, second_run: &Path) -> Result<()> {
    let load_artifacts = |run: &Path| -> Result<Value> {
        let file = fs::File::open(run.join("desktop-binary.json")).map_err(|error| {
            XtaskError::io(
                "debug_binary_missing",
                "read desktop binary evidence",
                error,
            )
        })?;
        serde_json::from_reader(file)
            .map_err(|error| XtaskError::failure("debug_binary_invalid", error.to_string()))
    };
    let primary_artifacts = load_artifacts(first_run)?;
    let browser_artifacts = load_artifacts(second_run)?;
    for field in ["/sha256", "/hostLibrary/sha256"] {
        let primary_hash = primary_artifacts.pointer(field).and_then(Value::as_str);
        if primary_hash.is_none()
            || primary_hash != browser_artifacts.pointer(field).and_then(Value::as_str)
        {
            return Err(XtaskError::failure(
                "debug_binary_changed",
                "desktop artifact changed between validation sessions",
            ));
        }
    }
    Ok(())
}

pub struct Session {
    child: Child,
    address: String,
    pub run_dir: PathBuf,
    /// Process spawn to debug-service ready file; excludes build, fixture setup,
    /// and artifact verification. This is not a content-first-present measurement.
    pub startup_ms: f64,
    model_fixture: Option<crate::model_fixture::ModelFixture>,
    capture_enabled: bool,
}

impl Session {
    pub(crate) fn verify_runtime_artifact(&self) -> Result {
        let artifact = read_replay_json(&self.run_dir.join("build-artifact.json"))?;
        source::verify_runtime_artifact(&artifact)?;
        write_json(
            &self.run_dir.join("runtime-artifact-verified.json"),
            &serde_json::json!({
                "verified": true, "verifiedAt": timestamp(),
                "executable": artifact["executable"], "executableSha256": artifact["executableSha256"],
                "hostLibrary": artifact["hostLibrary"],
            }),
        )
    }

    pub fn start(profile: &str) -> Result<Self> {
        Self::start_with_viewport(profile, None)
    }

    pub fn start_with_viewport(profile: &str, viewport: Option<(u32, u32, &str)>) -> Result<Self> {
        let root = repo_root()?;
        let run_dir = root
            .join("agent-debug-runs")
            .join(format!("lilia-{profile}-{}", timestamp()));
        fs::create_dir_all(&run_dir).map_err(|error| {
            XtaskError::io(
                "artifact_directory_failed",
                "create Agent Debug artifact directory",
                error,
            )
        })?;
        let ready = run_dir.join("ready.txt");
        let home = run_dir.join("home");
        fs::create_dir_all(&home).map_err(|error| {
            XtaskError::io("debug_home_failed", "create isolated LILIA_HOME", error)
        })?;
        if let Some((width, height, theme)) = viewport {
            write_json(
                &home.join("main-window-state.json"),
                &serde_json::json!({
                    "x": 40, "y": 40, "width": width, "height": height,
                    "scaleFactor": 1.0, "maximized": false
                }),
            )?;
            fs::write(home.join("appearance.theme"), theme).map_err(|error| {
                XtaskError::io("debug_theme_failed", "prepare screenshot theme", error)
            })?;
        }
        let revision = Command::new("git")
            .current_dir(&root)
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned());
        let dirty = Command::new("git")
            .current_dir(&root)
            .args(["status", "--porcelain"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned());
        let nana_checkout = root.parent().map(|parent| parent.join("NanaUI"));
        let nana_revision = nana_checkout
            .as_ref()
            .and_then(|path| {
                Command::new("git")
                    .current_dir(path)
                    .args(["rev-parse", "HEAD"])
                    .output()
                    .ok()
            })
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned());
        let local_path = source::local_nana_path(&root)?;
        let source_before = local_path.as_deref().map(source::fingerprint).transpose()?;
        let mut source_evidence = serde_json::json!({
            "revision": revision, "workingTree": dirty, "platform": std::env::consts::OS,
            "nanaUi": {"localPatch": local_path.is_some(), "localPath":local_path,
                "siblingRevision": nana_revision, "sourceFingerprint": source_before,
                "verifiedAfterBuild": false},
            "requestedViewport": viewport.map(|(width, height, theme)| serde_json::json!({"width":width,"height":height,"theme":theme}))
        });
        if profile == "performance" {
            source_evidence["performanceFixture"] = serde_json::json!({
                "id": "equivalence-performance-v1",
                "manifest": "tests/desktop/performance-v1.json",
                "sha256": source::file_sha256(&root.join("tests/desktop/performance-v1.json"))?,
            });
        }
        write_json(&run_dir.join("source.json"), &source_evidence)?;

        let performance_fixture = (profile == "performance").then(|| {
            (
                root.join("tests/desktop/performance-v1.json"),
                "equivalence-performance-v1",
            )
        });
        if let Some((fixture, _)) = &performance_fixture {
            run_command(
                crate::command("cargo").current_dir(&root).args([
                    "run",
                    "--locked",
                    "--quiet",
                    "-p",
                    "lilia-desktop",
                    "--example",
                    "equivalence_fixture",
                    "--",
                    "--manifest",
                    fixture.to_string_lossy().as_ref(),
                    "--home",
                    home.to_string_lossy().as_ref(),
                    "--identity",
                    "liliacode",
                ]),
                "seed native performance corpus",
            )?;
        }

        // 调试 socket 只在 debug_assertions 下存在，所以门禁只能测 dev profile。
        // 这意味着帧阈值判定的是未优化构建的成本。
        let build_output = crate::output(
            crate::command("cargo").current_dir(&root).args([
                "build",
                "--locked",
                "-p",
                "lilia-desktop",
                "--message-format=json-render-diagnostics",
            ]),
            "build native desktop",
        );
        let after_build = source::local_nana_path(&root).and_then(|path| {
            let fingerprint = path.as_deref().map(source::fingerprint).transpose()?;
            Ok((path, fingerprint))
        });
        let (path_after, source_after) = match after_build {
            Ok(source) => source,
            Err(error) => {
                source_evidence["nanaUi"]["sourceFingerprintErrorAfterBuild"] =
                    Value::String(error.to_string());
                write_json(&run_dir.join("source.json"), &source_evidence)?;
                return Err(XtaskError::failure(
                    "source_changed_during_build",
                    format!("cannot verify local NanaUI source after Cargo build: {error}"),
                ));
            }
        };
        source_evidence["nanaUi"]["sourceFingerprintAfterBuild"] =
            serde_json::to_value(&source_after).map_err(|error| {
                XtaskError::failure("artifact_serialize_failed", error.to_string())
            })?;
        let unchanged = if local_path == path_after {
            source::verify_unchanged(&source_before, &source_after)
        } else {
            Err(XtaskError::failure(
                "source_changed_during_build",
                "local NanaUI Cargo patch path changed during build",
            ))
        };
        source_evidence["nanaUi"]["verifiedAfterBuild"] = Value::Bool(unchanged.is_ok());
        write_json(&run_dir.join("source.json"), &source_evidence)?;
        unchanged?;
        let build_output = build_output?;
        let mut artifact = build_output
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|value| {
                value["reason"] == "compiler-artifact"
                    && value["target"]["name"] == "liliacode"
                    && value["executable"].is_string()
            })
            .ok_or_else(|| {
                XtaskError::failure(
                    "desktop_artifact_missing",
                    "Cargo did not report the desktop executable",
                )
            })?;
        let binary = PathBuf::from(
            artifact["executable"]
                .as_str()
                .expect("artifact executable"),
        );
        artifact["executableSha256"] = Value::String(source::file_sha256(&binary)?);
        source::record_host_artifact(&mut artifact, &build_output)?;
        write_json(&run_dir.join("build-artifact.json"), &artifact)?;
        let stdout = fs::File::create(run_dir.join("desktop.stdout.log")).map_err(|error| {
            XtaskError::io("debug_log_failed", "create desktop stdout log", error)
        })?;
        let stderr = fs::File::create(run_dir.join("desktop.stderr.log")).map_err(|error| {
            XtaskError::io("debug_log_failed", "create desktop stderr log", error)
        })?;
        let model_fixture = (profile == "agent-debug")
            .then(crate::model_fixture::ModelFixture::start)
            .transpose()?;
        let mut desktop = Command::new(&binary);
        desktop
            .current_dir(&root)
            .env("LILIA_HOME", &home)
            .env("LILIA_AGENT_DEBUG", "1")
            .env("LILIA_AGENT_DEBUG_ADDR", "127.0.0.1:0")
            .env("LILIA_AGENT_DEBUG_READY", &ready)
            .env("LILIA_AGENT_DEBUG_SEED", "1")
            .env_remove("LILIA_AGENT_DEBUG_RESUME")
            .env("LILIA_AGENT_DEBUG_EPHEMERAL_CREDENTIALS", "1")
            // Persists the kernel journal so a failed run leaves the ordered
            // record of mutations, events and mounts behind, not just logs.
            .env("LILIA_JOURNAL_PATH", run_dir.join("journal.jsonl"))
            .env(
                "LILIA_AGENT_DEBUG_MODEL_ENDPOINT",
                model_fixture
                    .as_ref()
                    .map_or(DISCARD_MODEL_ENDPOINT, |fixture| fixture.endpoint()),
            )
            // An inherited stdin would outlive a leaked desktop and wedge the caller's pipeline.
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        if profile == "agent-debug" {
            desktop.env("LILIA_AGENT_DEBUG_PAGINATION", "1");
        }
        if let Some((_, fixture_id)) = &performance_fixture {
            desktop.env("LILIA_EQUIVALENCE_FIXTURE_ID", fixture_id);
        }
        source::verify_runtime_artifact(&artifact)?;
        let started = Instant::now();
        let mut child = desktop.spawn().map_err(|error| {
            XtaskError::io(
                "desktop_launch_failed",
                &format!("launch {}", binary.display()),
                error,
            )
        })?;
        let address = match wait_ready(&ready) {
            Ok(address) => address,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(describe_startup_failure(error, &run_dir));
            }
        };
        let startup_ms = started.elapsed().as_secs_f64() * 1_000.0;
        Ok(Self {
            child,
            address,
            run_dir,
            startup_ms,
            model_fixture,
            capture_enabled: true,
        })
    }

    pub fn start_reusing(profile: &str) -> Result<Self> {
        Self::start_with_endpoint(profile, DISCARD_MODEL_ENDPOINT, true)
    }

    pub(super) fn start_with_endpoint(
        profile: &str,
        endpoint: &str,
        reuse_binary: bool,
    ) -> Result<Self> {
        let root = repo_root()?;
        let run_dir = root
            .join("agent-debug-runs")
            .join(format!("lilia-{profile}-{}", timestamp()));
        fs::create_dir_all(&run_dir).map_err(|error| {
            XtaskError::io(
                "artifact_directory_failed",
                "create Agent Debug artifact directory",
                error,
            )
        })?;
        let ready = run_dir.join("ready.txt");
        let home = run_dir.join("home");
        fs::create_dir_all(&home).map_err(|error| {
            XtaskError::io("debug_home_failed", "create isolated LILIA_HOME", error)
        })?;
        if !reuse_binary {
            run_command(
                crate::command("cargo").current_dir(&root).args([
                    "build",
                    "--locked",
                    "-p",
                    "lilia-desktop",
                ]),
                "build native desktop",
            )?;
        }
        let metadata = crate::output(
            crate::command("cargo").current_dir(&root).args([
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--no-deps",
            ]),
            "locate native desktop build",
        )?;
        let metadata: Value = serde_json::from_str(&metadata)
            .map_err(|error| XtaskError::failure("cargo_metadata_invalid", error.to_string()))?;
        let target = metadata
            .get("target_directory")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                XtaskError::failure(
                    "cargo_target_missing",
                    "Cargo did not report its target directory",
                )
            })?;
        let binary = Path::new(target)
            .join("debug")
            .join(crate::executable("liliacode"));
        let stdout = fs::File::create(run_dir.join("desktop.stdout.log")).map_err(|error| {
            XtaskError::io("debug_log_failed", "create desktop stdout log", error)
        })?;
        let stderr = fs::File::create(run_dir.join("desktop.stderr.log")).map_err(|error| {
            XtaskError::io("debug_log_failed", "create desktop stderr log", error)
        })?;
        let started = Instant::now();
        let mut desktop = Command::new(&binary);
        desktop
            .current_dir(&root)
            .env("LILIA_HOME", &home)
            .env("LILIA_AGENT_DEBUG", "1")
            .env("LILIA_AGENT_DEBUG_ADDR", "127.0.0.1:0")
            .env("LILIA_AGENT_DEBUG_READY", &ready)
            .env("LILIA_AGENT_DEBUG_SEED", "1")
            .env_remove("LILIA_AGENT_DEBUG_RESUME")
            .env("LILIA_AGENT_DEBUG_EPHEMERAL_CREDENTIALS", "1")
            .env("LILIA_JOURNAL_PATH", run_dir.join("journal.jsonl"))
            .env("LILIA_AGENT_DEBUG_MODEL_ENDPOINT", endpoint)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        let mut child = desktop.spawn().map_err(|error| {
            XtaskError::io(
                "desktop_launch_failed",
                &format!("launch {}", binary.display()),
                error,
            )
        })?;
        let address = match wait_ready(&ready) {
            Ok(address) => address,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(describe_startup_failure(error, &run_dir));
            }
        };
        let startup_ms = started.elapsed().as_secs_f64() * 1_000.0;
        let session = Self {
            child,
            address,
            run_dir,
            startup_ms,
            model_fixture: None,
            capture_enabled: true,
        };
        let host_library = binary.with_file_name(format!(
            "{}liliacode_host{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX,
        ));
        let mut artifacts = binary_fingerprint(&binary)?;
        artifacts["hostLibrary"] = binary_fingerprint(&host_library)?;
        artifacts["reused"] = Value::Bool(reuse_binary);
        artifacts["sourceRebuilt"] = Value::Bool(!reuse_binary);
        write_json(&session.run_dir.join("desktop-binary.json"), &artifacts)?;
        Ok(session)
    }

    pub fn request(&self, payload: &Value) -> Result<Value> {
        request(&self.address, payload)
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    fn capture(&self, output: &Path) -> Result {
        if self.capture_enabled {
            capture_window(self.pid(), output)
        } else {
            Ok(())
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn run() -> Result {
    run_with_capture(true)
}

pub fn run_with_capture(capture_enabled: bool) -> Result {
    run_with_viewport(capture_enabled, None)
}

pub fn run_browser_agent(reuse_binary: bool) -> Result {
    if !cfg!(target_os = "windows") {
        return Err(XtaskError::blocker(
            "windows_required",
            "Browser Agent acceptance requires Windows with a real native desktop",
        ));
    }
    let result = browser_agent::run(reuse_binary)?;
    println!("agent-debug browser-agent: {result}");
    Ok(())
}

pub fn run_matrix() -> Result {
    for (width, height) in [(960, 600), (1440, 900)] {
        for theme in ["light", "dark"] {
            run_with_viewport(true, Some((width, height, theme)))?;
        }
    }
    Ok(())
}

fn run_with_viewport(capture_enabled: bool, viewport: Option<(u32, u32, &str)>) -> Result {
    if !cfg!(any(target_os = "windows", target_os = "macos")) {
        return Err(XtaskError::blocker(
            "desktop_required",
            "Agent Debug acceptance requires macOS or Windows with a real WGPU desktop",
        ));
    }
    if capture_enabled {
        require_interactive_desktop_session()?;
    }
    let mut session = Session::start_with_viewport("agent-debug", viewport)?;
    session.capture_enabled = capture_enabled;
    let observation = session.request(&serde_json::json!({ "command": "observe" }))?;
    require_ok(&observation, "observe")?;
    let visible = wait_ui(&session)?;
    write_json(&session.run_dir.join("ui-before.json"), &visible)?;
    let settings_target = visible
        .pointer("/snapshot/targets")
        .and_then(Value::as_array)
        .and_then(|targets| {
            targets
                .iter()
                .filter_map(|target| target["id"].as_str())
                .find(|id| id.starts_with("lilia.ui.navigation.") && id.contains("settings"))
        })
        .ok_or_else(|| {
            XtaskError::failure(
                "settings_control_missing",
                "settings navigation has no mounted control",
            )
        })?;
    let action = session.request(&serde_json::json!({
        "command": "ui-click", "targetId": settings_target
    }))?;
    require_ok(&action, "click settings")?;
    let replay = wait_observation(&session, |value| {
        value
            .pointer("/observation/page")
            .and_then(Value::as_str)
            .is_some_and(|page| page.contains("settings"))
    })?;
    require_ok(&replay, "observe after action")?;
    let mut scenarios = Vec::new();
    for tab in [
        "appearance",
        "project",
        "provider",
        "agent",
        "quota",
        "extensions",
        "remote",
        "desktop",
        "data",
        "about",
    ] {
        reveal_scrolled_control(
            &session,
            "lilia.ui.settings.scroll",
            &format!("lilia.ui.settings.tab.{tab}"),
        )?;
        let action = session.request(&serde_json::json!({"command": "ui-click",
            "targetId": format!("lilia.ui.settings.tab.{tab}")}))?;
        require_ok(&action, "activate settings tab")?;
        let state = wait_observation(&session, |value| {
            value
                .pointer("/observation/settingsTab")
                .and_then(Value::as_str)
                == Some(tab)
                && (tab != "quota"
                    || (value["observation"]["quotaBusy"] == false
                        && value["observation"]["quotaError"].is_null()
                        && value["observation"]["quotaRecordCount"]
                            .as_u64()
                            .unwrap_or(0)
                            >= 21))
        })?;
        if tab == "quota" {
            write_json(&session.run_dir.join("settings-quota-state.json"), &state)?;
            quota_replay::replay(&session)?;
        }
        if tab == "remote" {
            replay_remote(&session)?;
        }
        if tab == "appearance" {
            replay_sidebar_modes(&session)?;
        }
        let controls = wait_ui(&session)?;
        let capture = session.run_dir.join(format!("settings-{tab}.png"));
        session.capture(&capture)?;
        scenarios.push(serde_json::json!({"tab": tab, "state": state, "ui": controls}));
    }
    write_json(&session.run_dir.join("scenarios.json"), &scenarios.into())?;
    replay_extensions(&session)?;
    wait_target(&session, "lilia.ui.settings.back")?;
    require_ok(
        &session.request(
            &serde_json::json!({"command": "ui-click", "targetId": "lilia.ui.settings.back"}),
        )?,
        "return from settings",
    )?;
    wait_observation(&session, |value| {
        value.pointer("/observation/page").and_then(Value::as_str) != Some("settings")
    })?;
    wait_target(&session, "lilia.ui.new-conversation")?;
    require_ok(
        &session.request(
            &serde_json::json!({"command": "ui-click", "targetId": "lilia.ui.new-conversation"}),
        )?,
        "open new conversation",
    )?;
    wait_target(&session, "lilia.task-session.composer.input")?;
    session.capture(&session.run_dir.join("empty-conversation.png"))?;
    require_ok(&session.request(&serde_json::json!({"command": "ui-input", "targetId": "lilia.task-session.composer.input", "text": "中文草稿\n第二行\n#Composer"}))?, "search conversation references")?;
    let reference = "lilia.ui.composer.reference-result-native-agent-debug-task";
    wait_target(&session, reference)?;
    require_ok(
        &session.request(&serde_json::json!({"command": "ui-click", "targetId": reference}))?,
        "select conversation reference",
    )?;
    wait_observation(&session, |value| {
        value
            .pointer("/observation/composerConversationReferenceCount")
            .and_then(Value::as_u64)
            == Some(1)
    })?;
    let reasoning = replay_composer_dropdown(
        &session,
        "lilia.ui.composer.reasoning",
        "composer-reasoning-steps.json",
    )?;
    wait_observation(&session, |value| {
        value
            .pointer("/observation/composerReasoning")
            .and_then(Value::as_str)
            == Some(reasoning.as_str())
    })?;
    let model = replay_composer_dropdown(
        &session,
        "lilia.ui.composer.model",
        "composer-model-steps.json",
    )?;
    let composer = wait_observation(&session, |value| {
        value
            .pointer("/observation/composerModel")
            .and_then(Value::as_str)
            == Some(model.as_str())
    })?;
    write_json(&session.run_dir.join("composer.json"), &composer)?;
    session.capture(&session.run_dir.join("composer.png"))?;
    wait_target(&session, "lilia.ui.send")?;
    require_ok(
        &session.request(&serde_json::json!({"command":"ui-key", "targetId":"lilia.task-session.composer.input", "key":"Enter"}))?,
        "send referenced conversation with Enter",
    )?;
    let fixture = session
        .model_fixture
        .as_ref()
        .expect("agent-debug model fixture");
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    let model_request = loop {
        if let Some(request) = fixture
            .requests()
            .into_iter()
            .find(|request| request["messages"].to_string().contains("中文草稿"))
        {
            break request;
        }
        if Instant::now() >= deadline {
            write_json(
                &session.run_dir.join("send-failure.json"),
                &session.request(&serde_json::json!({"command":"observe"}))?,
            )?;
            return Err(XtaskError::failure(
                "model_request_missing",
                "normal send did not reach the loopback provider",
            ));
        }
        thread::sleep(Duration::from_millis(100));
    };
    write_json(&session.run_dir.join("model-request.json"), &model_request)?;
    if model_request["model"] != composer["observation"]["composerModel"] {
        return Err(XtaskError::failure(
            "model_selection_not_applied",
            "request model differs from the visible composer selection",
        ));
    }
    let referenced_context = model_request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|message| message["content"].as_str())
        .filter_map(|content| {
            content.strip_prefix(
                "Product-provided workspace and turn context (authoritative for this turn): ",
            )
        })
        .filter_map(|context| serde_json::from_str::<Value>(context).ok())
        .any(|context| {
            context["conversationReferences"]
                .as_array()
                .is_some_and(|references| {
                    references.len() == 1
                        && references[0]["taskId"] == "native-agent-debug-task"
                        && references[0]["title"] == "验证 Native Composer 与时间线"
                })
        });
    if !referenced_context {
        return Err(XtaskError::failure(
            "reference_content_missing",
            "selected structured conversation reference is absent from the actual model request",
        ));
    }
    let completion = wait_observation(&session, |value| {
        value
            .pointer("/observation/turnState")
            .and_then(Value::as_str)
            == Some("completed")
            && value
                .pointer("/observation/timelineEventCount")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                > 1
            && value
                .pointer("/observation/activeTurnId")
                .is_none_or(Value::is_null)
    })?;
    write_json(&session.run_dir.join("completed-turn.json"), &completion)?;
    replay_markdown_image(&session)?;
    session.capture(&session.run_dir.join("completed-turn.png"))?;
    write_json(
        &session.run_dir.join("completed-ui.json"),
        &session.request(&serde_json::json!({"command":"ui-observe"}))?,
    )?;
    let fork_target = reveal_timeline_target(&session, ".fork")?;
    require_ok(
        &session.request(&serde_json::json!({"command":"ui-click", "targetId":fork_target}))?,
        "choose reply fork anchor",
    )?;
    wait_target(&session, "lilia.ui.composer.branch-clear")?;
    require_ok(
        &session.request(
            &serde_json::json!({"command":"ui-click", "targetId":"lilia.ui.composer.branch-clear"}),
        )?,
        "clear reply fork anchor",
    )?;
    replay_fork_history(&session)?;
    replay_review_targets(&session)?;
    replay_todo(&session)?;
    replay_pending_approval(&session)?;
    replay_browser(&session)?;
    replay_project_sessions_and_handoff(&session)?;
    replay_automation(&session)?;
    automation_replay::replay(&mut session)?;
    knowledge_replay::replay(&mut session)?;
    extensions_execution::replay(&session)?;
    memory_popup_replay::replay(&mut session)?;
    let errors = session.request(&serde_json::json!({ "command": "recent-errors" }))?;
    require_ok(&errors, "recent-errors")?;
    let secret_canary = "sk-native-agent-debug-fixture";
    for (label, artifact) in [
        ("observation", &observation),
        ("action", &action),
        ("replay", &replay),
        ("errors", &errors),
    ] {
        if artifact.to_string().contains(secret_canary) {
            return Err(XtaskError::failure(
                "agent_debug_secret_leak",
                format!("{label} artifact contains the secret canary"),
            ));
        }
    }
    let journal = read_journal(&session.run_dir.join("journal.jsonl"))?;
    write_json(&session.run_dir.join("observe.json"), &observation)?;
    write_json(&session.run_dir.join("action.json"), &action)?;
    write_json(&session.run_dir.join("replay.json"), &replay)?;
    write_json(&session.run_dir.join("errors.json"), &errors)?;
    session.capture(&session.run_dir.join("desktop.png"))?;
    session.verify_runtime_artifact()?;
    write_json(
        &session.run_dir.join("summary.json"),
        &serde_json::json!({
            "ok": true,
            "protocol": "lilia-agent-debug-v1",
            "artifacts": session.run_dir,
            "journalRecords": journal,
            "platform": std::env::consts::OS,
            "interactionPath": "mounted-runtime-control",
            "visualEvidence": if capture_enabled { "captured" } else { "not_collected" },
        }),
    )?;
    println!("agent-debug: ok ({})", session.run_dir.display());
    Ok(())
}

/// `PrintWindow` asks the window to redraw into our bitmap, so the capture stays
/// correct while the desktop is occluded and never picks up other applications.
///
/// Two Windows details matter. `MainWindowHandle` is unreliable because the
/// process also owns tool and helper windows, so the largest visible top-level
/// window wins instead. And the capturing process must opt into per-monitor DPI
/// awareness, or Windows virtualises `GetClientRect` down to 96dpi and
/// `PrintWindow` silently crops the physical window to that smaller bitmap.
const CAPTURE_SCRIPT: &str = r#"
param([int]$ProcessId, [string]$Output, [long]$NativeWindowId = 0, [switch]$ListOnly, [int]$LogicalWidth = 0, [int]$LogicalHeight = 0)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class LiliaCapture {
    public delegate bool EnumProc(IntPtr handle, IntPtr param);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr param);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr handle, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr handle);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr handle, out RECT rect);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr handle, out RECT rect);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr handle, IntPtr context, uint flags);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr handle, int command);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr handle, IntPtr insertAfter, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr handle);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr context);
    public struct RECT { public int Left, Top, Right, Bottom; }
    public static long[] VisibleWindows(uint target) {
        var windows = new List<long>();
        EnumWindows(delegate(IntPtr handle, IntPtr param) {
            uint owner;
            GetWindowThreadProcessId(handle, out owner);
            if (owner == target && IsWindowVisible(handle)) { windows.Add(handle.ToInt64()); }
            return true;
        }, IntPtr.Zero);
        return windows.ToArray();
    }
    public static IntPtr LargestVisibleWindow(uint target) {
        IntPtr best = IntPtr.Zero;
        long bestArea = 0;
        EnumWindows(delegate(IntPtr handle, IntPtr param) {
            uint owner;
            GetWindowThreadProcessId(handle, out owner);
            if (owner != target || !IsWindowVisible(handle)) { return true; }
            RECT rect;
            if (!GetClientRect(handle, out rect)) { return true; }
            long area = (long)(rect.Right - rect.Left) * (rect.Bottom - rect.Top);
            if (area > bestArea) { bestArea = area; best = handle; }
            return true;
        }, IntPtr.Zero);
        return best;
    }
}
'@
# DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
[LiliaCapture]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
if ($ListOnly) {
    ConvertTo-Json -InputObject @([LiliaCapture]::VisibleWindows($ProcessId)) -Compress
    exit
}
$handle = [IntPtr]::Zero
for ($attempt = 0; $attempt -lt 60; $attempt++) {
    if ($NativeWindowId -ne 0) {
        if ([LiliaCapture]::VisibleWindows($ProcessId) -notcontains $NativeWindowId) { throw 'requested window is not visible or belongs to another process' }
        $handle = [IntPtr]$NativeWindowId
    } else {
        $handle = [LiliaCapture]::LargestVisibleWindow($ProcessId)
    }
    if ($handle -ne [IntPtr]::Zero) { break }
    Start-Sleep -Milliseconds 250
}
if ($handle -eq [IntPtr]::Zero) { throw 'desktop window not ready' }
[LiliaCapture]::ShowWindow($handle, 9) | Out-Null
$previous = New-Object LiliaCapture+RECT
$dpi = 0
$resized = $false
$restore = $false
try {
    if ($LogicalWidth -ne 0 -or $LogicalHeight -ne 0) {
        if ($LogicalWidth -le 0 -or $LogicalHeight -le 0) { throw 'desktop window resize failed: LogicalWidth and LogicalHeight are required together' }
        if (-not [LiliaCapture]::GetWindowRect($handle, [ref]$previous)) { throw 'desktop window resize failed: previous bounds unavailable' }
        $dpi = [LiliaCapture]::GetDpiForWindow($handle)
        if ($dpi -eq 0) { throw 'desktop window resize failed: dpi unavailable' }
        $client = New-Object LiliaCapture+RECT
        if (-not [LiliaCapture]::GetClientRect($handle, [ref]$client)) { throw 'desktop window resize failed: client bounds unavailable' }
        $frameW = ($previous.Right - $previous.Left) - ($client.Right - $client.Left)
        $frameH = ($previous.Bottom - $previous.Top) - ($client.Bottom - $client.Top)
        $targetW = [int][Math]::Round($LogicalWidth * $dpi / 96.0)
        $targetH = [int][Math]::Round($LogicalHeight * $dpi / 96.0)
        if (-not [LiliaCapture]::SetWindowPos($handle, [IntPtr]::Zero, 0, 0, $targetW + $frameW, $targetH + $frameH, 0x16)) {
            throw 'desktop window resize failed'
        }
        $resized = $true
        $restore = $true
    }
    Start-Sleep -Milliseconds 700
    $rect = New-Object LiliaCapture+RECT
    [LiliaCapture]::GetClientRect($handle, [ref]$rect) | Out-Null
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -le 0 -or $height -le 0) {
        if ($resized) { throw 'desktop window resize failed: client bounds unavailable' }
        throw 'desktop window has invalid bounds'
    }
    if ($resized) {
        $targetW = [int][Math]::Round($LogicalWidth * $dpi / 96.0)
        $targetH = [int][Math]::Round($LogicalHeight * $dpi / 96.0)
        if ([Math]::Abs($width - $targetW) -gt 1 -or [Math]::Abs($height - $targetH) -gt 1) {
            throw "desktop window resize failed: client ${width}x${height}, expected ${targetW}x${targetH}"
        }
    }
    $bitmap = New-Object Drawing.Bitmap $width, $height
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $context = $graphics.GetHdc()
    # PW_RENDERFULLCONTENT: required for composited GPU surfaces.
    $printed = [LiliaCapture]::PrintWindow($handle, $context, 2)
    $graphics.ReleaseHdc($context)
    $graphics.Dispose()
    if (-not $printed) { $bitmap.Dispose(); throw 'PrintWindow refused the desktop window' }
    $bitmap.Save($Output, [Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
    $restore = $false
    $result = @{ windowId = $handle.ToInt64(); pid = $ProcessId; pixelWidth = $width; pixelHeight = $height; platform = 'windows'; captureStatus = 'complete' }
    if ($resized) {
        $result.logicalWidth = $LogicalWidth
        $result.logicalHeight = $LogicalHeight
        $result.scaleFactor = $dpi / 96.0
        $result.previousOuterWidth = $previous.Right - $previous.Left
        $result.previousOuterHeight = $previous.Bottom - $previous.Top
    }
    $result | ConvertTo-Json -Compress
} finally {
    if ($restore) {
        [LiliaCapture]::SetWindowPos($handle, [IntPtr]::Zero, 0, 0, ($previous.Right - $previous.Left), ($previous.Bottom - $previous.Top), 0x16) | Out-Null
    }
}
"#;

fn browser_target(observation: &Value, suffix: &str) -> Option<String> {
    observation
        .pointer("/observation/visibleTargetIds")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find(|target| target.starts_with("lilia.browser.") && target.ends_with(suffix))
        .map(str::to_owned)
}

fn wait_browser(
    session: &Session,
    name: &str,
    predicate: impl Fn(&Value) -> bool,
) -> Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let state = session.request(&serde_json::json!({"command": "observe"}))?;
        require_ok(&state, "observe browser")?;
        if predicate(&state) {
            return Ok(state);
        }
        if state
            .pointer("/observation/iabError")
            .is_some_and(|error| !error.is_null())
            || Instant::now() >= deadline
        {
            write_json(
                &session.run_dir.join(format!("browser-{name}-blocked.json")),
                &state,
            )?;
            capture_window(
                session.pid(),
                &session.run_dir.join(format!("browser-{name}-blocked.png")),
            )?;
            write_json(
                &session.run_dir.join("browser-acceptance.json"),
                &serde_json::json!({
                    "passed": false,
                    "blockedAt": name,
                    "recovery": "Use the browser Retry or Install runtime action; this scenario did not pass."
                }),
            )?;
            return Err(XtaskError::blocker(
                "browser_acceptance_blocked",
                format!(
                    "browser {name} did not become available; inspect browser-{name}-blocked.json/png and use its runtime recovery action"
                ),
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn record_browser(session: &Session, name: &str, state: &Value) -> Result {
    if state.to_string().contains("sk-native-agent-debug-fixture") {
        return Err(XtaskError::failure(
            "agent_debug_secret_leak",
            "browser observation contains the secret canary",
        ));
    }
    write_json(&session.run_dir.join(format!("browser-{name}.json")), state)?;
    capture_window(
        session.pid(),
        &session.run_dir.join(format!("browser-{name}.png")),
    )
}

pub(crate) fn capture_window(pid: u32, output: &Path) -> Result {
    capture_selected_window(pid, output, None, None)
}

fn capture_named_window(pid: u32, output: &Path, _title: &str) -> Result {
    capture_window(pid, output)
}

pub(super) fn capture_resized_window(pid: u32, output: &Path, size: [u32; 2]) -> Result {
    capture_selected_window(pid, output, None, Some(size))
}

fn capture_selected_window(
    pid: u32,
    output: &Path,
    native_window: Option<u64>,
    size: Option<[u32; 2]>,
) -> Result {
    require_interactive_desktop_session()?;
    if cfg!(target_os = "macos") {
        if size.is_some() {
            return Err(XtaskError::blocker(
                "capture_window_resize_failed",
                "resized window capture requires Windows",
            ));
        }
        return capture_macos_window(pid, output, native_window);
    }
    // A `-Command` string cannot carry this much inline C#, so the script goes to
    // a temporary file rather than beside the screenshot the caller asked for.
    let script = std::env::temp_dir().join("liliacode-capture-window.ps1");
    fs::write(&script, CAPTURE_SCRIPT).map_err(|error| {
        XtaskError::io(
            "agent_debug_capture_script_failed",
            "write capture script",
            error,
        )
    })?;
    let script_path = script.display().to_string();
    let output_path = output.display().to_string();
    let pid_text = pid.to_string();
    let window_text = native_window.unwrap_or(0).to_string();
    let width_text = size.map(|value| value[0].to_string());
    let height_text = size.map(|value| value[1].to_string());
    let mut command = crate::command("powershell.exe");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        &script_path,
        "-ProcessId",
        &pid_text,
        "-Output",
        &output_path,
        "-NativeWindowId",
        &window_text,
    ]);
    if let (Some(width), Some(height)) = (&width_text, &height_text) {
        command.args(["-LogicalWidth", width, "-LogicalHeight", height]);
    }
    let metadata =
        crate::output(&mut command, "capture native desktop screenshot").map_err(|error| {
            if size.is_some()
                && error.code == "command_failed"
                && error.message.contains("desktop window resize failed")
            {
                XtaskError::blocker("capture_window_resize_failed", error.message)
            } else {
                error
            }
        })?;
    if !output.is_file() || output.metadata().map(|value| value.len()).unwrap_or(0) == 0 {
        return Err(XtaskError::failure(
            "agent_debug_screenshot_missing",
            format!("screenshot was not created: {}", output.display()),
        ));
    }
    let metadata: Value = serde_json::from_str(metadata.trim())
        .map_err(|error| XtaskError::failure("capture_window_invalid", error.to_string()))?;
    let image = image::open(output)
        .map_err(|error| XtaskError::failure("capture_image_invalid", error.to_string()))?;
    validate_capture_content(&image)?;
    if let Some([width, height]) = size {
        let scale = metadata
            .get("scaleFactor")
            .and_then(Value::as_f64)
            .unwrap_or(1.0);
        let expected_width = (f64::from(width) * scale).round() as u32;
        let expected_height = (f64::from(height) * scale).round() as u32;
        if image.width().abs_diff(expected_width) > 1
            || image.height().abs_diff(expected_height) > 1
        {
            let _ = fs::remove_file(output);
            return Err(XtaskError::blocker(
                "capture_window_resize_failed",
                format!(
                    "screenshot {} is {}x{}, expected {}x{} for logical {width}x{height}",
                    output.display(),
                    image.width(),
                    image.height(),
                    expected_width,
                    expected_height
                ),
            ));
        }
    }
    write_json(&output.with_extension("window.json"), &metadata)
}

pub(crate) fn require_interactive_desktop_session() -> Result {
    #[cfg(target_os = "macos")]
    {
        let state = Command::new("/usr/sbin/ioreg")
            .args(["-n", "Root", "-d1"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned());
        if state.as_deref().is_some_and(macos_session_is_locked) {
            return Err(XtaskError::blocker(
                "desktop_session_locked",
                "the macOS user session is locked; unlock it before running screenshot or frame-performance acceptance",
            ));
        }
    }
    Ok(())
}

#[cfg(any(target_os = "macos", test))]
fn macos_session_is_locked(state: &str) -> bool {
    state.lines().any(|line| {
        line.contains("\"CGSSessionScreenIsLocked\"=Yes")
            || line.contains("\"IOConsoleLocked\" = Yes")
    })
}

const MACOS_WINDOW_SCRIPT: &str = r#"
import Foundation
import CoreGraphics
let pid = Int(CommandLine.arguments[1])!
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
let candidates = windows.filter { ($0[kCGWindowOwnerPID as String] as? Int) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }
let selection = CommandLine.arguments[2]
if selection == "list" {
    let ids = candidates.compactMap { $0[kCGWindowNumber as String] as? Int }
    print(String(data: try JSONSerialization.data(withJSONObject: ids), encoding: .utf8)!)
    exit(0)
}
func area(_ window: [String: Any]) -> Double {
    let bounds = window[kCGWindowBounds as String] as? [String: Double] ?? [:]
    return (bounds["Width"] ?? 0) * (bounds["Height"] ?? 0)
}
let requested = Int(selection)!
let selected = requested == 0 ? candidates.max(by: { area($0) < area($1) }) : candidates.first { ($0[kCGWindowNumber as String] as? Int) == requested }
guard let window = selected, area(window) > 0 else { exit(2) }
let result: [String: Any] = ["windowId": window[kCGWindowNumber as String]!, "pid": pid, "bounds": window[kCGWindowBounds as String]!, "screenCaptureAccess": CGPreflightScreenCaptureAccess()]
print(String(data: try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys]), encoding: .utf8)!)
"#;

fn visible_native_windows(pid: u32) -> Result<Vec<u64>> {
    let macos = cfg!(target_os = "macos");
    let script = std::env::temp_dir().join(if macos {
        "liliacode-native-windows.swift"
    } else {
        "liliacode-native-windows.ps1"
    });
    fs::write(
        &script,
        if macos {
            MACOS_WINDOW_SCRIPT
        } else {
            CAPTURE_SCRIPT
        },
    )
    .map_err(|error| {
        XtaskError::io(
            "window_inventory_failed",
            "write native window inventory",
            error,
        )
    })?;
    let mut command = crate::command(if macos {
        "/usr/bin/swift"
    } else {
        "powershell.exe"
    });
    if macos {
        command.arg(&script).arg(pid.to_string()).arg("list");
    } else {
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(&script)
            .arg("-ProcessId")
            .arg(pid.to_string())
            .arg("-ListOnly");
    }
    let output = crate::output(&mut command, "enumerate visible native windows")?;
    serde_json::from_str(output.trim())
        .map_err(|error| XtaskError::failure("window_inventory_invalid", error.to_string()))
}

fn unique_added_window(before: &[u64], after: &[u64]) -> Result<u64> {
    let before: std::collections::BTreeSet<_> = before.iter().copied().collect();
    let added: std::collections::BTreeSet<_> = after
        .iter()
        .copied()
        .filter(|id| !before.contains(id))
        .collect();
    match added.iter().copied().collect::<Vec<_>>().as_slice() {
        [id] if *id != 0 => Ok(*id),
        _ => Err(XtaskError::failure(
            "popup_native_identity_ambiguous",
            format!("expected one newly visible native window; before={before:?}, after={after:?}"),
        )),
    }
}

fn capture_task_popup(
    session: &Session,
    runtime_window: u64,
    task: &str,
    before: &[u64],
) -> Result {
    if !session.capture_enabled {
        return Ok(());
    }
    let after = visible_native_windows(session.pid())?;
    let native_window = unique_added_window(before, &after)?;
    let snapshot = session.request(&serde_json::json!({
        "command": "ui-observe", "windowId": runtime_window.to_string()
    }))?;
    require_ok(&snapshot, "observe popup immediately before native capture")?;
    let output = session.run_dir.join("task-popup.png");
    write_json(&session.run_dir.join("task-popup-ui.json"), &snapshot)?;
    capture_selected_window(session.pid(), &output, Some(native_window), None)?;
    let confirmed = visible_native_windows(session.pid())?;
    if !confirmed.contains(&native_window) {
        return Err(XtaskError::failure(
            "popup_native_window_disappeared",
            "the captured popup is no longer visible in the same process",
        ));
    }
    let metadata_path = output.with_extension("window.json");
    let mut metadata: Value =
        serde_json::from_slice(&fs::read(&metadata_path).map_err(|error| {
            XtaskError::io(
                "popup_capture_metadata_failed",
                "read captured window identity",
                error,
            )
        })?)
        .map_err(|error| {
            XtaskError::failure("popup_capture_metadata_invalid", error.to_string())
        })?;
    if metadata["windowId"].as_u64() != Some(native_window)
        || metadata["pid"].as_u64() != Some(u64::from(session.pid()))
    {
        return Err(XtaskError::failure(
            "popup_capture_identity_mismatch",
            "the captured native identity does not match the opened popup",
        ));
    }
    metadata["runtimeWindowId"] = runtime_window.into();
    metadata["nativeWindowId"] = native_window.into();
    metadata["taskId"] = task.into();
    metadata["identityMethod"] = "unique-native-window-added-by-normal-popup-action".into();
    metadata["nativeWindowsBefore"] = serde_json::json!(before);
    metadata["nativeWindowsAfter"] = serde_json::json!(after);
    metadata["nativeWindowsAfterCapture"] = serde_json::json!(confirmed);
    write_json(&metadata_path, &metadata)
}

fn capture_macos_window(pid: u32, output: &Path, native_window: Option<u64>) -> Result {
    let script = output.with_extension("capture.swift");
    fs::write(&script, MACOS_WINDOW_SCRIPT).map_err(|error| {
        XtaskError::io(
            "capture_script_failed",
            "write macOS window selector",
            error,
        )
    })?;
    let result = crate::output(
        crate::command("/usr/bin/swift")
            .arg(&script)
            .arg(pid.to_string())
            .arg(native_window.unwrap_or(0).to_string()),
        "identify native macOS window",
    )?;
    let mut metadata: Value = serde_json::from_str(result.trim())
        .map_err(|error| XtaskError::failure("capture_window_invalid", error.to_string()))?;
    let window = metadata["windowId"].as_u64().ok_or_else(|| {
        XtaskError::failure(
            "capture_window_missing",
            "no visible window belongs to the desktop process",
        )
    })?;
    metadata["captureStatus"] = "pending".into();
    write_json(&output.with_extension("window.json"), &metadata)?;
    let mut attempts = Vec::new();
    for attempt in 1..=3 {
        match crate::run(
            crate::command("/usr/sbin/screencapture")
                .args(["-x", "-o", "-l", &window.to_string()])
                .arg(output),
            "capture native macOS window",
        ) {
            Ok(()) => {
                attempts.push(serde_json::json!({"attempt": attempt, "status": "complete"}));
                metadata["captureAttempts"] = attempts.into();
                break;
            }
            Err(error) => {
                let after = crate::output(
                    crate::command("/usr/bin/swift")
                        .arg(&script)
                        .arg(pid.to_string())
                        .arg(native_window.unwrap_or(0).to_string()),
                    "inspect native window after capture failure",
                )
                .ok()
                .and_then(|result| serde_json::from_str::<Value>(result.trim()).ok())
                .unwrap_or(Value::Null);
                attempts.push(serde_json::json!({
                    "attempt": attempt, "status": "failed", "error": error.to_string(),
                    "windowAfterFailure": after
                }));
                let same_surface = after["windowId"].as_u64() == Some(window)
                    && after["pid"].as_u64() == Some(pid.into())
                    && after["bounds"] == metadata["bounds"]
                    && after["screenCaptureAccess"].as_bool() == Some(true);
                metadata["captureAttempts"] = attempts.clone().into();
                if attempt == 3 || !same_surface {
                    metadata["captureStatus"] = "failed".into();
                    metadata["captureError"] = error.to_string().into();
                    let _ = write_json(&output.with_extension("window.json"), &metadata);
                    return Err(error);
                }
                write_json(&output.with_extension("window.json"), &metadata)?;
                thread::sleep(Duration::from_millis(200));
            }
        }
    }
    let image = image::open(output)
        .map_err(|error| XtaskError::failure("capture_image_invalid", error.to_string()))?;
    if image.width() < 100 || image.height() < 100 {
        return Err(XtaskError::failure(
            "capture_image_empty",
            "captured window has no usable surface",
        ));
    }
    validate_capture_content(&image)?;
    metadata["captureStatus"] = "complete".into();
    metadata["pixelWidth"] = image.width().into();
    metadata["pixelHeight"] = image.height().into();
    metadata["scaleFactor"] =
        (image.width() as f64 / metadata["bounds"]["Width"].as_f64().unwrap_or(1.0)).into();
    metadata["platform"] = "macos".into();
    write_json(&output.with_extension("window.json"), &metadata)
}

fn wait_ui(session: &Session) -> Result<Value> {
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    loop {
        let response = session.request(&serde_json::json!({"command": "ui-observe"}))?;
        require_ok(&response, "observe mounted UI")?;
        if response
            .pointer("/snapshot/targets")
            .and_then(Value::as_array)
            .is_some_and(|targets| !targets.is_empty())
        {
            return Ok(response);
        }
        if Instant::now() >= deadline {
            return Err(XtaskError::failure(
                "ui_not_ready",
                "no mounted controls became visible",
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

pub(crate) fn wait_target(session: &Session, target: &str) -> Result<Value> {
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    loop {
        let response = session.request(&serde_json::json!({"command": "ui-observe"}))?;
        require_ok(&response, "observe target")?;
        if response
            .pointer("/snapshot/targets")
            .and_then(Value::as_array)
            .is_some_and(|targets| targets.iter().any(|entry| entry["id"] == target))
        {
            return Ok(response);
        }
        if Instant::now() >= deadline {
            write_json(&session.run_dir.join("ui-failure.json"), &response)?;
            let _ = session.capture(&session.run_dir.join("ui-failure.png"));
            return Err(XtaskError::failure(
                "ui_target_not_visible",
                format!("{target} is not exposed in the window"),
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn wait_matching_target(session: &Session, predicate: impl Fn(&str) -> bool) -> Result<String> {
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    loop {
        let response = session.request(&serde_json::json!({"command":"ui-observe"}))?;
        require_ok(&response, "observe matching UI target")?;
        if let Some(id) = response
            .pointer("/snapshot/targets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry["id"].as_str())
            .find(|id| predicate(id))
        {
            return Ok(id.to_owned());
        }
        if Instant::now() >= deadline {
            write_json(&session.run_dir.join("ui-failure.json"), &response)?;
            let _ = session.capture(&session.run_dir.join("ui-failure.png"));
            return Err(XtaskError::failure(
                "ui_target_not_visible",
                "expected action is not exposed after the state transition",
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn replay_composer_dropdown(session: &Session, target: &str, artifact: &str) -> Result<String> {
    let before = wait_target(session, target)?;
    let dropdown = before
        .pointer("/snapshot/targets")
        .and_then(Value::as_array)
        .and_then(|targets| targets.iter().find(|entry| entry["id"] == target))
        .and_then(|entry| entry.get("dropdown"))
        .ok_or_else(|| {
            XtaskError::failure(
                "dropdown_diagnostics_missing",
                format!("{target} has no Dropdown diagnostics"),
            )
        })?;
    let selected = dropdown["selection"].as_str().ok_or_else(|| {
        XtaskError::failure(
            "dropdown_selection_missing",
            format!("{target} is not a selected single Dropdown"),
        )
    })?;
    let enabled = dropdown["options"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|option| option["disabled"].as_bool() == Some(false))
        .filter_map(|option| option["value"].as_str())
        .collect::<Vec<_>>();
    let current = enabled
        .iter()
        .position(|value| *value == selected)
        .ok_or_else(|| {
            XtaskError::failure(
                "dropdown_selection_missing",
                format!("{target} selected value is not enabled"),
            )
        })?;
    let expected = enabled[(current + 1) % enabled.len()].to_owned();
    if expected == selected {
        return Err(XtaskError::failure(
            "dropdown_alternative_missing",
            format!("{target} has no different keyboard choice"),
        ));
    }
    let mut steps = vec![serde_json::json!({"step":"before","ui":before,
        "business":session.request(&serde_json::json!({"command":"observe"}))?})];
    write_json(&session.run_dir.join(artifact), &serde_json::json!(steps))?;
    for (step, request) in [
        (
            "click",
            serde_json::json!({"command":"ui-click","targetId":target}),
        ),
        (
            "ArrowDown",
            serde_json::json!({"command":"ui-key","targetId":target,"key":"ArrowDown"}),
        ),
        (
            "Enter",
            serde_json::json!({"command":"ui-key","targetId":target,"key":"Enter"}),
        ),
    ] {
        require_ok(&session.request(&request)?, step)?;
    }
    let ui = wait_ui(session)?;
    let business = session.request(&serde_json::json!({"command":"observe"}))?;
    require_ok(&business, "observe Dropdown selection")?;
    steps.push(serde_json::json!({"step":"after-continuous-keys","ui":ui,"business":business}));
    write_json(&session.run_dir.join(artifact), &serde_json::json!(steps))?;
    Ok(expected)
}

fn wait_observation(session: &Session, predicate: impl Fn(&Value) -> bool) -> Result<Value> {
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    loop {
        let response = session.request(&serde_json::json!({"command": "observe"}))?;
        require_ok(&response, "observe product state")?;
        if predicate(&response) {
            return Ok(response);
        }
        if Instant::now() >= deadline {
            write_json(&session.run_dir.join("state-failure.json"), &response)?;
            let _ = session.capture(&session.run_dir.join("state-failure.png"));
            return Err(XtaskError::failure(
                "ui_action_no_effect",
                "mounted action did not produce the expected product state",
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn turn_context_from_request(request: &Value) -> Option<Value> {
    request["messages"]
        .as_array()?
        .iter()
        .rev()
        .filter_map(|message| {
            let content = message["content"].as_str()?;
            let context = content.strip_prefix(
                "Product-provided workspace and turn context (authoritative for this turn): ",
            )?;
            serde_json::from_str::<Value>(context).ok()
        })
        .next()
}

fn wait_model_request(
    session: &Session,
    after: usize,
    predicate: impl Fn(&Value) -> bool,
) -> Result<Value> {
    let fixture = session.model_fixture.as_ref().expect("loopback fixture");
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    loop {
        if let Some(request) = fixture
            .requests()
            .into_iter()
            .skip(after)
            .find(|request| turn_context_from_request(request).is_some() && predicate(request))
        {
            return Ok(request);
        }
        if Instant::now() >= deadline {
            write_json(
                &session.run_dir.join("request-failure.json"),
                &serde_json::json!({
                    "requests":fixture.requests().into_iter().skip(after).collect::<Vec<_>>(),
                    "state":session.request(&serde_json::json!({"command":"observe"}))?
                }),
            )?;
            session.capture(&session.run_dir.join("request-failure.png"))?;
            return Err(XtaskError::failure(
                "model_request_missing",
                "normal UI submission did not produce the expected provider request",
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn wait_completed_turn(session: &Session) -> Result<Value> {
    wait_observation(session, |value| {
        value["observation"]["turnState"] == "completed"
            && value["observation"]["activeTurnId"].is_null()
            && value["observation"]["composerLength"] == 0
    })
}

fn send_marked_turn(session: &Session, marker: &str, label: &str) -> Result<Value> {
    let after = session
        .model_fixture
        .as_ref()
        .expect("loopback fixture")
        .requests()
        .len();
    ui_input(session, "lilia.task-session.composer.input", marker)?;
    wait_observation(session, |value| {
        value["observation"]["composerContentSha256"] == composer_digest(marker)
    })?;
    require_ok(
        &session.request(&serde_json::json!({"command":"ui-key",
        "targetId":"lilia.task-session.composer.input", "key":"Enter"}))?,
        "send marked history turn",
    )?;
    let request = wait_model_request(session, after, |request| {
        request["messages"].as_array().is_some_and(|messages| {
            messages
                .iter()
                .rev()
                .find(|message| message["role"] == "user")
                .is_some_and(|message| {
                    message["content"]
                        .as_str()
                        .is_some_and(|content| content.contains(marker))
                })
        })
    })?;
    write_json(
        &session.run_dir.join(format!("fork-{label}-request.json")),
        &request,
    )?;
    let state = wait_completed_turn(session)?;
    write_json(
        &session.run_dir.join(format!("fork-{label}-completed.json")),
        &state,
    )?;
    Ok(request)
}

fn select_replay_model(session: &Session) -> Result {
    for control in ["reasoning", "model"] {
        let target = format!("lilia.ui.composer.{control}");
        ui_click(session, &target)?;
        for key in ["ArrowDown", "Enter"] {
            require_ok(
                &session.request(&serde_json::json!({"command":"ui-key",
                "targetId":target,"key":key}))?,
                "select explicit replay model settings",
            )?;
        }
    }
    wait_observation(session, |value| {
        value["observation"]["composerModel"].is_string()
            && value["observation"]["composerReasoning"].is_string()
    })?;
    Ok(())
}

fn replay_fork_history(session: &Session) -> Result {
    ui_click(session, "lilia.ui.new-conversation")?;
    let first = send_marked_turn(session, "NATIVE_PARITY_FORK_A_20260903", "a")?;
    let selection = turn_context_from_request(&first).expect("first turn context")
        ["automaticSelection"]
        .clone();
    if selection["source"] != "auto"
        || selection["model"] != first["model"]
        || selection["reasoningEffort"] != first["reasoning_effort"]
    {
        return Err(XtaskError::failure(
            "automatic_selection_not_applied",
            "automatic model decision differs from the actual first turn request",
        ));
    }
    write_json(
        &session.run_dir.join("automatic-model-selection.json"),
        &selection,
    )?;
    let anchor = reveal_timeline_target(session, ".fork")?;
    let second = send_marked_turn(session, "NATIVE_PARITY_FORK_B_20260903", "b")?;
    let messages = second["messages"].to_string();
    if !messages.contains("NATIVE_PARITY_FORK_A_20260903")
        || !messages.contains("NATIVE_PARITY_REPLY_A_20260903")
    {
        return Err(XtaskError::failure(
            "fork_fixture_history_missing",
            "second ordinary turn did not inherit the first turn",
        ));
    }
    reveal_scrolled_control(session, "lilia.ui.timeline.scroll", &anchor)?;
    ui_click(session, &anchor)?;
    wait_target(session, "lilia.ui.composer.branch-clear")?;
    session.capture(&session.run_dir.join("fork-anchor-selected.png"))?;
    let forked = send_marked_turn(session, "NATIVE_PARITY_FORK_C_20260903", "c")?;
    let messages = forked["messages"].to_string();
    let context = turn_context_from_request(&forked).expect("captured turn context");
    if !messages.contains("NATIVE_PARITY_FORK_A_20260903")
        || !messages.contains("NATIVE_PARITY_REPLY_A_20260903")
        || messages.contains("NATIVE_PARITY_FORK_B_20260903")
        || messages.contains("NATIVE_PARITY_REPLY_B_20260903")
        || context["sessionBranch"]["mode"] != "fork"
    {
        return Err(XtaskError::failure(
            "fork_history_not_truncated",
            "forked HTTP request did not retain only history through the selected first reply",
        ));
    }
    write_json(
        &session.run_dir.join("fork-history-verified.json"),
        &serde_json::json!({
            "anchorTarget":anchor, "firstTurnRetained":true, "laterTurnExcluded":true,
            "sessionBranch":context["sessionBranch"]
        }),
    )?;
    session.capture(&session.run_dir.join("fork-completed.png"))?;
    Ok(())
}

fn replay_review_targets(session: &Session) -> Result {
    for (label, steps, target, input) in [
        (
            "changes",
            0,
            serde_json::json!({"type":"uncommittedChanges"}),
            None,
        ),
        (
            "branch",
            1,
            serde_json::json!({"type":"baseBranch","branch":"parity-review-base"}),
            Some("parity-review-base"),
        ),
        (
            "commit",
            2,
            serde_json::json!({"type":"commit","sha":"0123456789abcdef0123456789abcdef01234567"}),
            Some("0123456789abcdef0123456789abcdef01234567"),
        ),
    ] {
        ui_click(session, "lilia.ui.new-conversation")?;
        select_replay_model(session)?;
        ui_input(session, "lilia.task-session.composer.input", "/review")?;
        wait_target(session, "lilia.ui.completion.slash-review")?;
        require_ok(
            &session.request(&serde_json::json!({"command":"ui-key",
            "targetId":"lilia.task-session.composer.input","key":"Tab"}))?,
            "select review slash completion",
        )?;
        wait_target(session, "lilia.ui.composer.review-target")?;
        if steps > 0 {
            ui_click(session, "lilia.ui.composer.review-target")?;
            for _ in 0..steps {
                require_ok(
                    &session.request(&serde_json::json!({"command":"ui-key",
                    "targetId":"lilia.ui.composer.review-target","key":"ArrowDown"}))?,
                    "choose review target",
                )?;
            }
            require_ok(
                &session.request(&serde_json::json!({"command":"ui-key",
                "targetId":"lilia.ui.composer.review-target","key":"Enter"}))?,
                "confirm review target",
            )?;
        }
        if let Some(input) = input {
            ui_input(session, "lilia.ui.composer.review-value", input)?;
        }
        session.capture(&session.run_dir.join(format!("review-{label}-target.png")))?;
        let after = session
            .model_fixture
            .as_ref()
            .expect("loopback fixture")
            .requests()
            .len();
        ui_click(session, "lilia.ui.composer.review-submit")?;
        let request = wait_model_request(session, after, |request| {
            turn_context_from_request(request)
                .is_some_and(|context| context["workflow"]["type"] == "lilia_review")
        })?;
        write_json(
            &session.run_dir.join(format!("review-{label}-request.json")),
            &request,
        )?;
        let context = turn_context_from_request(&request).expect("review turn context");
        if context["workflow"]["target"] != target || context["workflow"]["delivery"] != "inline" {
            return Err(XtaskError::failure(
                "review_target_not_applied",
                format!(
                    "{label} selection differs from the actual workflow in the provider request"
                ),
            ));
        }
        write_json(
            &session
                .run_dir
                .join(format!("review-{label}-completed.json")),
            &wait_completed_turn(session)?,
        )?;
    }
    Ok(())
}

fn replay_project_sessions_and_handoff(session: &Session) -> Result {
    const PROJECT: &str = "native-agent-debug-zz-pagination";
    const LAST: &str = "native-pagination-105";
    ui_click(session, &format!("lilia.ui.sidebar.row.{PROJECT}"))?;
    wait_observation(session, |value| {
        value["observation"]["selectedProject"] == PROJECT
            && value["observation"]["taskCount"] == 105
    })?;
    for page in 0..6 {
        let first = page * 20 + 1;
        wait_target(
            session,
            &format!("lilia.ui.project-card.session-native-pagination-{first:03}"),
        )?;
        write_json(
            &session
                .run_dir
                .join(format!("sessions-page-{}.json", page + 1)),
            &session.request(&serde_json::json!({"command":"ui-observe"}))?,
        )?;
        if page < 5 {
            ui_click(session, "lilia.ui.composer.session-next")?;
        }
    }
    session.capture(&session.run_dir.join("sessions-page-6.png"))?;
    ui_click(session, &format!("lilia.ui.project-card.session-{LAST}"))?;
    let selected = wait_observation(session, |value| {
        value["observation"]["selectedTask"] == LAST
    })?;
    write_json(
        &session.run_dir.join("sessions-selected-105.json"),
        &selected,
    )?;
    wait_target(session, "lilia.task-session.composer.input")?;

    let workspace = session.run_dir.join("handoff-workspace");
    fs::create_dir_all(&workspace).map_err(|error| {
        XtaskError::io(
            "handoff_workspace_failed",
            "create isolated handoff workspace",
            error,
        )
    })?;
    let payload = session.run_dir.join("handoff.json");
    write_json(
        &payload,
        &serde_json::json!({
            "protocol":"lilia-code-task-handoff", "version":1,
            "id":"native-ui-handoff", "createdAt":"2026-09-03T00:00:00Z",
            "title":"正常导航恢复草稿", "kind":"repository",
            "repository":{"fullName":"fixture/native-ui", "worktreePath":workspace, "branch":"main"},
            "source":{"application":"LiliaGithub", "route":"/fixture/native-ui"},
            "problem":"请检查此仓库，保留这段提示词供编辑。",
            "relatedFiles":[], "acceptanceCriteria":["草稿由用户确认后发送"]
        }),
    )?;
    launch_handoff_cli(session, &payload, "first")?;
    let receipt_path = PathBuf::from(format!("{}.receipt.json", payload.display()));
    let receipt = read_replay_json(&receipt_path)?;
    let task = receipt["taskId"].as_str().ok_or_else(|| {
        XtaskError::failure(
            "handoff_receipt_missing_task",
            "normal CLI did not report its imported task",
        )
    })?;
    let imported = wait_observation(session, |value| {
        value["observation"]["selectedTask"] == task
            && value["observation"]["composerLength"]
                .as_u64()
                .is_some_and(|length| length > 20)
            && value["observation"]["activeTurnId"].is_null()
    })?;
    wait_target(session, "lilia.task-session.composer.input")?;
    write_json(&session.run_dir.join("handoff-imported.json"), &imported)?;
    session.capture(&session.run_dir.join("handoff-imported.png"))?;
    let edited = "Edited handoff draft\nUser decides when to send.";
    let edited_digest = composer_digest(edited);
    ui_input(session, "lilia.task-session.composer.input", edited)?;
    let changed = wait_observation(session, |value| {
        value["observation"]["composerLength"] == edited.len()
            && value["observation"]["composerContentSha256"] == edited_digest
            && value["observation"]["composerRevision"].as_u64()
                > imported["observation"]["composerRevision"].as_u64()
    })?;
    // Leave the task through the normal project control, so the second CLI must navigate again.
    ui_click(session, &format!("lilia.ui.sidebar.row.{PROJECT}"))?;
    wait_observation(session, |value| {
        value["observation"]["selectedTask"].is_null()
    })?;
    launch_handoff_cli(session, &payload, "duplicate")?;
    let duplicate = read_replay_json(&receipt_path)?;
    if duplicate["taskId"] != task || duplicate["status"] != "accepted" {
        return Err(XtaskError::failure(
            "handoff_duplicate_changed_task",
            "reopening the same handoff did not preserve its task identity",
        ));
    }
    let reopened = wait_observation(session, |value| {
        value["observation"]["selectedTask"] == task
            && value["observation"]["composerLength"] == edited.len()
            && value["observation"]["composerContentSha256"] == edited_digest
            && value["observation"]["composerRevision"]
                == changed["observation"]["composerRevision"]
            && value["observation"]["activeTurnId"].is_null()
    })?;
    wait_target(session, "lilia.task-session.composer.input")?;
    write_json(&session.run_dir.join("handoff-reopened.json"), &reopened)?;
    session.capture(&session.run_dir.join("handoff-reopened.png"))?;
    Ok(())
}

fn read_replay_json(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).map_err(|error| {
        XtaskError::io(
            "replay_artifact_read_failed",
            "read UI replay artifact",
            error,
        )
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|error| XtaskError::failure("replay_artifact_invalid", error.to_string()))
}

fn launch_handoff_cli(session: &Session, payload: &Path, label: &str) -> Result {
    let artifact = read_replay_json(&session.run_dir.join("build-artifact.json"))?;
    let executable = artifact["executable"].as_str().ok_or_else(|| {
        XtaskError::failure(
            "desktop_artifact_missing",
            "desktop executable missing from build artifact",
        )
    })?;
    let stdout = fs::File::create(session.run_dir.join(format!("handoff-{label}.stdout.log")))
        .map_err(|error| {
            XtaskError::io("handoff_log_failed", "create handoff stdout log", error)
        })?;
    let stderr = fs::File::create(session.run_dir.join(format!("handoff-{label}.stderr.log")))
        .map_err(|error| {
            XtaskError::io("handoff_log_failed", "create handoff stderr log", error)
        })?;
    let mut child = Command::new(executable)
        .arg("--task-handoff")
        .arg(payload)
        .env("LILIA_HOME", session.run_dir.join("home"))
        .env_remove("LILIA_AGENT_DEBUG")
        .env_remove("LILIA_AGENT_DEBUG_SEED")
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| {
            XtaskError::io("handoff_launch_failed", "launch normal handoff CLI", error)
        })?;
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(XtaskError::failure(
                    "handoff_cli_failed",
                    format!("normal handoff CLI exited with {status}"),
                ));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(XtaskError::io(
                    "handoff_wait_failed",
                    "wait for handoff CLI forwarding",
                    error,
                ));
            }
            Ok(None) => {}
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(XtaskError::failure(
                "handoff_cli_timeout",
                "normal CLI did not forward to the existing desktop within the timeout",
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn replay_todo(session: &Session) -> Result {
    let before = session.request(&serde_json::json!({"command":"observe"}))?;
    let count = before
        .pointer("/observation/todoCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    ui_click(session, "lilia.ui.composer.plus")?;
    ui_click(session, "lilia.ui.plus.new-guide")?;
    ui_input(session, "lilia.ui.todo.input", "验证引导编辑\n第二行")?;
    ui_click(session, "lilia.ui.todo.save")?;
    wait_observation(session, |state| {
        state
            .pointer("/observation/todoCount")
            .and_then(Value::as_u64)
            == Some(count + 1)
            && state
                .pointer("/observation/todoTitles")
                .and_then(Value::as_array)
                .is_some_and(|titles| titles.iter().any(|title| title == "验证引导编辑\n第二行"))
    })?;
    let priority = wait_matching_target(session, |id| {
        id.starts_with("lilia.ui.todo.") && id.ends_with("-priority")
    })?;
    ui_click(session, &priority)?;
    let prioritized = wait_observation(session, |state| {
        state
            .pointer("/observation/todoPriorities")
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value == "low"))
    })?;
    write_json(&session.run_dir.join("todo-priority.json"), &prioritized)?;
    let edit = priority.trim_end_matches("-priority").to_owned() + "-edit";
    ui_click(session, &edit)?;
    ui_input(session, "lilia.ui.todo.input", "取消后不能保存的草稿")?;
    ui_click(session, "lilia.ui.todo.cancel")?;
    let unchanged = wait_observation(session, |state| {
        state
            .pointer("/observation/todoTitles")
            .and_then(Value::as_array)
            .is_some_and(|titles| {
                titles.iter().any(|title| title == "验证引导编辑\n第二行")
                    && !titles.iter().any(|title| title == "取消后不能保存的草稿")
            })
    })?;
    write_json(
        &session.run_dir.join("todo-cancelled-edit.json"),
        &unchanged,
    )?;
    session.capture(&session.run_dir.join("todo.png"))?;
    let delete = priority.trim_end_matches("-priority").to_owned() + "-delete";
    ui_click(session, &delete)?;
    wait_observation(session, |state| {
        state
            .pointer("/observation/todoCount")
            .and_then(Value::as_u64)
            == Some(count)
    })?;
    Ok(())
}

fn remote_authority(session: &Session) -> Result<Value> {
    let read = || -> rusqlite::Result<Value> {
        let db = rusqlite::Connection::open_with_flags(
            session.run_dir.join("home/db/product.db"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let name: String = db.query_row(
            "SELECT value FROM remote_control_settings WHERE key = 'pc_name'",
            [],
            |row| row.get(0),
        )?;
        let (device_id, trusted, revoked): (String, bool, Option<i64>) = db.query_row(
            "SELECT id, trusted, revoked_at FROM remote_control_trusted_devices WHERE endpoint_id = 'native-agent-debug-android'",
            [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let active: i64 = db.query_row(
            "SELECT COUNT(*) FROM remote_control_pairing_tickets WHERE consumed_at IS NULL AND expires_at > ?1",
            [timestamp() as i64], |row| row.get(0),
        )?;
        Ok(
            serde_json::json!({"pcName":name,"fixtureDeviceId":device_id,"fixtureDeviceTrusted":trusted,"fixtureDeviceRevoked":revoked.is_some(),"activeTicketCount":active}),
        )
    };
    read().map_err(|error| XtaskError::failure("remote_authority_read_failed", error.to_string()))
}

fn replay_remote(session: &Session) -> Result {
    let ready = wait_observation(session, |value| {
        value["observation"]["remoteBusy"] == false
            && value["observation"]["remoteError"].is_null()
            && value["observation"]["remoteTrustedDeviceCount"] == 1
    })?;
    let name = "Native UI 配对验收电脑";
    reveal_scrolled_control(
        session,
        "lilia.ui.settings.content-scroll",
        "lilia.ui.settings.remote-name",
    )?;
    ui_input(session, "lilia.ui.settings.remote-name", name)?;
    ui_click(session, "lilia.ui.settings.remote-name-save")?;
    wait_observation(session, |value| {
        value["observation"]["remotePcName"] == name && value["observation"]["remoteBusy"] == false
    })?;
    let saved = remote_authority(session)?;
    if saved["pcName"] != name {
        return Err(XtaskError::failure(
            "remote_name_not_persisted",
            "normal name save did not reach authority",
        ));
    }
    reveal_scrolled_control(
        session,
        "lilia.ui.settings.content-scroll",
        "lilia.ui.settings.remote-pair",
    )?;
    ui_click(session, "lilia.ui.settings.remote-pair")?;
    wait_observation(session, |value| {
        value["observation"]["remotePairingActive"] == true
            && value["observation"]["remoteBusy"] == false
    })?;
    let pairing = remote_authority(session)?;
    if pairing["activeTicketCount"].as_i64().unwrap_or(0) < 1 {
        return Err(XtaskError::failure(
            "remote_pairing_not_persisted",
            "normal QR generation created no active ticket",
        ));
    }
    reveal_scrolled_control(
        session,
        "lilia.ui.settings.content-scroll",
        "lilia.ui.settings.remote-cancel",
    )?;
    session.capture(&session.run_dir.join("remote-pairing.png"))?;
    ui_click(session, "lilia.ui.settings.remote-cancel")?;
    wait_observation(session, |value| {
        value["observation"]["remotePairingActive"] == false
            && value["observation"]["remoteBusy"] == false
    })?;
    let cancelled = remote_authority(session)?;
    if cancelled["activeTicketCount"] != 0 {
        return Err(XtaskError::failure(
            "remote_pairing_cancel_not_persisted",
            "cancelled ticket remains active",
        ));
    }
    let device_id = cancelled["fixtureDeviceId"].as_str().ok_or_else(|| {
        XtaskError::failure(
            "remote_device_id_missing",
            "fixture authority has no device id",
        )
    })?;
    let revoke = format!("lilia.ui.settings.remote-revoke-{device_id}");
    reveal_scrolled_control(session, "lilia.ui.settings.content-scroll", &revoke)?;
    ui_click(session, &revoke)?;
    wait_target(session, "lilia.ui.confirm.cancel")?;
    ui_click(session, "lilia.ui.confirm.cancel")?;
    wait_target(session, &revoke)?;
    let revoke_cancelled = remote_authority(session)?;
    if revoke_cancelled["fixtureDeviceTrusted"] != true
        || revoke_cancelled["fixtureDeviceRevoked"] != false
    {
        return Err(XtaskError::failure(
            "remote_revoke_cancel_changed_authority",
            "cancel changed trusted device",
        ));
    }
    ui_click(session, &revoke)?;
    wait_target(session, "lilia.ui.confirm.commit")?;
    ui_click(session, "lilia.ui.confirm.commit")?;
    wait_observation(session, |value| {
        value["observation"]["remoteTrustedDeviceCount"] == 0
            && value["observation"]["remoteBusy"] == false
    })?;
    reveal_scrolled_control(
        session,
        "lilia.ui.settings.content-scroll",
        "lilia.ui.settings.remote-refresh",
    )?;
    ui_click(session, "lilia.ui.settings.remote-refresh")?;
    let final_state = wait_observation(session, |value| {
        value["observation"]["remoteTrustedDeviceCount"] == 0
            && value["observation"]["remoteBusy"] == false
            && value["observation"]["remoteError"].is_null()
    })?;
    let revoked = remote_authority(session)?;
    if revoked["fixtureDeviceTrusted"] != false || revoked["fixtureDeviceRevoked"] != true {
        return Err(XtaskError::failure(
            "remote_revoke_not_persisted",
            "normal confirmed revoke did not persist",
        ));
    }
    session.capture(&session.run_dir.join("remote-device-revoked.png"))?;
    write_json(
        &session.run_dir.join("remote-local-verified.json"),
        &serde_json::json!({
            "initial":ready,"nameSaved":saved,"pairing":pairing,"pairingCancelled":cancelled,
            "revokeCancelled":revoke_cancelled,"revoked":revoked,"final":final_state,"realAndroidPairing":false
        }),
    )
}

fn replay_sidebar_modes(session: &Session) -> Result {
    let selector = "lilia.ui.settings.appearance-sidebar";
    wait_observation(session, |value| {
        value["observation"]["sidebarDisplayMode"] == "grouped"
    })?;
    let mut evidence = Vec::new();
    for (mode, key) in [("unified", "ArrowDown"), ("grouped", "ArrowUp")] {
        reveal_scrolled_control(session, "lilia.ui.settings.content-scroll", selector)?;
        ui_click(session, selector)?;
        for key in [key, "Enter"] {
            require_ok(
                &session.request(
                    &serde_json::json!({"command":"ui-key", "targetId":selector,"key":key}),
                )?,
                "choose sidebar display mode",
            )?;
        }
        let state = wait_observation(session, |value| {
            value["observation"]["sidebarDisplayMode"] == mode
        })?;
        let authority = wait_extension_file(
            &session.run_dir.join("home/sidebar-display-mode.json"),
            |value| value == mode,
        )?;
        ui_click(session, "lilia.ui.settings.back")?;
        wait_observation(session, |value| {
            value["observation"]["page"] != "settings"
                && value["observation"]["sidebarDisplayMode"] == mode
        })?;
        let ui = wait_ui(session)?;
        let targets = ui
            .pointer("/snapshot/targets")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                XtaskError::failure("sidebar_targets_missing", "sidebar has no mounted targets")
            })?;
        let project_visible = targets
            .iter()
            .any(|target| target["id"] == "lilia.ui.sidebar.row.native-agent-debug-project");
        let rows_visible = targets.iter().any(|target| {
            target["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("lilia.ui.sidebar.row."))
        });
        if !rows_visible || project_visible != (mode == "grouped") {
            return Err(XtaskError::failure(
                "sidebar_mode_projection_mismatch",
                "normal settings selection did not change the mounted sidebar hierarchy",
            ));
        }
        session.capture(
            &session
                .run_dir
                .join(format!("appearance-sidebar-{mode}.png")),
        )?;
        evidence.push(
            serde_json::json!({"mode":mode,"state":state,"authority":authority,"ui":ui.clone()}),
        );
        let target = targets
            .iter()
            .filter_map(|target| target["id"].as_str())
            .find(|id| id.starts_with("lilia.ui.navigation.") && id.contains("settings"))
            .ok_or_else(|| {
                XtaskError::failure(
                    "settings_control_missing",
                    "sidebar has no settings navigation control",
                )
            })?;
        ui_click(session, target)?;
        reveal_scrolled_control(
            session,
            "lilia.ui.settings.scroll",
            "lilia.ui.settings.tab.appearance",
        )?;
        ui_click(session, "lilia.ui.settings.tab.appearance")?;
        wait_observation(session, |value| {
            value["observation"]["settingsTab"] == "appearance"
        })?;
    }
    write_json(
        &session.run_dir.join("appearance-sidebar-modes.json"),
        &Value::Array(evidence),
    )
}

fn replay_pending_approval(session: &Session) -> Result {
    replay_plan_response(session, false)?;
    replay_plan_response(session, true)
}

fn replay_plan_response(session: &Session, revise: bool) -> Result {
    let prefix = if revise { "plan-revise" } else { "approval" };
    let call_id = if revise {
        "plan-revise-fixture-call"
    } else {
        "approval-fixture-call"
    };
    let feedback = "请保留现有配置，只检查界面。\n第二步先验证草稿恢复，再继续。";
    let fixture = session.model_fixture.as_ref().expect("loopback fixture");
    let request_start = fixture.requests().len();
    fixture.hold_responses(true);
    ui_input(
        session,
        "lilia.task-session.composer.input",
        if revise {
            "验证修改计划与草稿恢复"
        } else {
            "验证审批与草稿恢复"
        },
    )?;
    require_ok(&session.request(&serde_json::json!({"command":"ui-key", "targetId":"lilia.task-session.composer.input", "key":"Enter"}))?, "request plan approval")?;
    let draft = "审批之后继续编辑的草稿\n保留第二行";
    ui_input(session, "lilia.task-session.composer.input", draft)?;
    fixture.hold_responses(false);
    let approve = wait_matching_target(session, |id| {
        id.starts_with("lilia.ui.pending.pending-plan-approve-")
    })?;
    let pending = session.request(&serde_json::json!({"command":"ui-observe"}))?;
    if pending
        .pointer("/snapshot/targets")
        .and_then(Value::as_array)
        .is_some_and(|targets| {
            targets
                .iter()
                .any(|target| target["id"] == "lilia.task-session.composer.input")
        })
    {
        return Err(XtaskError::failure(
            "pending_composer_not_replaced",
            "ordinary composer remained exposed during approval",
        ));
    }
    let action_target = if revise {
        let target = approve.replace("pending-plan-approve-", "pending-plan-revise-");
        if pending
            .pointer("/snapshot/targets")
            .and_then(Value::as_array)
            .is_some_and(|targets| targets.iter().any(|entry| entry["id"] == target))
        {
            return Err(XtaskError::failure(
                "empty_plan_feedback_executable",
                "plan revision was interactive before any feedback was entered",
            ));
        }
        ui_input(session, "lilia.ui.pending.draft", feedback)?;
        wait_target(session, &target)?;
        target
    } else {
        approve
    };
    session.capture(&session.run_dir.join(format!("pending-{prefix}.png")))?;
    ui_click(session, &action_target)?;
    wait_target(session, "lilia.task-session.composer.input")?;
    let completed = wait_observation(session, |value| {
        value
            .pointer("/observation/turnState")
            .and_then(Value::as_str)
            == Some("completed")
            && value
                .pointer("/observation/activeTurnId")
                .is_none_or(Value::is_null)
    })?;
    if completed["observation"]["composerLength"].as_u64() != Some(draft.len() as u64)
        || completed["observation"]["composerContentSha256"] != composer_digest(draft)
    {
        return Err(XtaskError::failure(
            "pending_draft_lost",
            "approval replaced the independently edited ordinary draft",
        ));
    }
    let resumed = fixture
        .requests()
        .into_iter()
        .skip(request_start)
        .rev()
        .find(|request| {
            request["messages"].as_array().is_some_and(|messages| {
                messages
                    .iter()
                    .any(|message| message["role"] == "tool" && message["tool_call_id"] == call_id)
            })
        })
        .ok_or_else(|| {
            XtaskError::failure(
                "approval_resume_missing",
                "normal approval did not resume the same model tool call",
            )
        })?;
    let result = resumed["messages"]
        .as_array()
        .and_then(|messages| {
            messages
                .iter()
                .rev()
                .find(|message| message["role"] == "tool" && message["tool_call_id"] == call_id)
        })
        .and_then(|message| message["content"].as_str())
        .and_then(|content| serde_json::from_str::<Value>(content).ok());
    let expected = if revise {
        serde_json::json!({"action":"revise", "feedback":feedback})
    } else {
        serde_json::json!({"action":"approve"})
    };
    if result.as_ref() != Some(&expected) {
        return Err(XtaskError::failure(
            "plan_response_mismatch",
            "normal plan action did not preserve its exact tool result",
        ));
    }
    write_json(
        &session.run_dir.join(format!("{prefix}-verified.json")),
        &serde_json::json!({
            "toolCallId":call_id, "toolResult":result, "ordinaryDraftSha256":composer_digest(draft),
            "ordinaryDraftRestored":true, "sameToolCall":true
        }),
    )?;
    write_json(
        &session
            .run_dir
            .join(format!("{prefix}-resumed-request.json")),
        &resumed,
    )?;
    write_json(
        &session.run_dir.join(format!("{prefix}-completed.json")),
        &completed,
    )?;
    session.capture(&session.run_dir.join(format!("{prefix}-draft-restored.png")))?;
    ui_input(session, "lilia.task-session.composer.input", "")?;
    Ok(())
}

fn replay_browser(session: &Session) -> Result {
    if !cfg!(target_os = "macos") {
        return Ok(());
    }
    ui_click(session, "lilia.ui.sidebar.more")?;
    let menu = wait_target(session, "lilia.ui.menu.window")?;
    session.capture(&session.run_dir.join("task-menu.png"))?;
    let item = menu
        .pointer("/snapshot/menu/items")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find(|item| item["value"] == "more-browser"))
        .ok_or_else(|| {
            XtaskError::failure(
                "browser_menu_missing",
                "normal task menu has no browser entry",
            )
        })?;
    require_ok(&session.request(&serde_json::json!({"command":"ui-click-at", "targetId":"lilia.ui.menu.window", "x":item["x"].to_string(), "y":item["y"].to_string()}))?, "open browser from task menu")?;
    wait_target(session, "lilia.ui.iab.address")?;
    let fixture = session.model_fixture.as_ref().expect("loopback fixture");
    for page in ["one", "two"] {
        let url = fixture.page_url(page);
        ui_input(session, "lilia.ui.iab.address", &url)?;
        ui_click(session, "lilia.ui.iab.navigate")?;
        wait_observation(session, |value| {
            value.pointer("/observation/iabUrl").and_then(Value::as_str) == Some(url.as_str())
                && value
                    .pointer("/observation/iabBrowserReady")
                    .and_then(Value::as_bool)
                    == Some(true)
        })?;
    }
    ui_click(session, "lilia.ui.iab.back")?;
    let first_url = fixture.page_url("one");
    wait_observation(session, |value| {
        value.pointer("/observation/iabUrl").and_then(Value::as_str) == Some(first_url.as_str())
            && value
                .pointer("/observation/iabBrowserReady")
                .and_then(Value::as_bool)
                == Some(true)
    })?;
    session.capture(&session.run_dir.join("browser.png"))?;
    ui_click(session, "lilia.ui.iab.capture")?;
    let captured = wait_observation(session, |value| {
        value
            .pointer("/observation/composerAttachmentCount")
            .and_then(Value::as_u64)
            == Some(1)
            && value
                .pointer("/observation/composerLength")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                > 0
    })?;
    write_json(&session.run_dir.join("browser-captured.json"), &captured)?;
    Ok(())
}

fn reveal_timeline_target(session: &Session, suffix: &str) -> Result<String> {
    for delta in std::iter::once(10_000.0).chain(std::iter::repeat_n(-200.0, 32)) {
        let ui = session.request(&serde_json::json!({"command":"ui-observe"}))?;
        if let Some(id) = ui
            .pointer("/snapshot/targets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry["id"].as_str())
            .find(|id| id.starts_with("lilia.ui.timeline.") && id.ends_with(suffix))
        {
            return Ok(id.to_owned());
        }
        require_ok(&session.request(&serde_json::json!({"command":"ui-scroll", "targetId":"lilia.ui.timeline.scroll", "deltaY":delta.to_string()}))?, "reveal timeline action")?;
        thread::sleep(Duration::from_millis(100));
    }
    wait_matching_target(session, |id| {
        id.starts_with("lilia.ui.timeline.") && id.ends_with(suffix)
    })
}

fn replay_markdown_image(session: &Session) -> Result {
    let mut image_target = None;
    for delta in std::iter::once(10_000.0).chain(std::iter::repeat_n(-180.0, 32)) {
        let ui = session.request(&serde_json::json!({"command":"ui-observe"}))?;
        image_target = ui
            .pointer("/snapshot/targets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find_map(|entry| {
                let image = entry["images"].as_array()?.first()?;
                Some((
                    entry["id"].as_str()?.to_owned(),
                    image["x"].as_f64()?,
                    image["y"].as_f64()?,
                ))
            });
        if image_target.is_some() {
            break;
        }
        require_ok(&session.request(&serde_json::json!({"command":"ui-scroll", "targetId":"lilia.ui.timeline.scroll", "deltaY":delta.to_string()}))?, "reveal rendered Markdown image")?;
        thread::sleep(Duration::from_millis(100));
    }
    let Some((target, x, y)) = image_target else {
        write_json(
            &session.run_dir.join("ui-failure.json"),
            &session.request(&serde_json::json!({"command":"ui-observe"}))?,
        )?;
        session.capture(&session.run_dir.join("ui-failure.png"))?;
        return Err(XtaskError::failure(
            "markdown_image_not_rendered",
            "no exposed image primitive appeared in the reply",
        ));
    };
    require_ok(&session.request(&serde_json::json!({"command":"ui-click-at", "targetId":target, "x":x.to_string(), "y":y.to_string()}))?, "open rendered Markdown image")?;
    wait_target(session, "lilia.ui.image.viewer")?;
    session.capture(&session.run_dir.join("markdown-image.png"))?;
    require_ok(&session.request(&serde_json::json!({"command":"ui-key", "targetId":"lilia.ui.image.viewer", "key":"Escape"}))?, "close image viewer")?;
    wait_target(session, "lilia.task-session.composer.input")?;
    Ok(())
}

fn replay_automation(session: &Session) -> Result {
    let navigation = wait_matching_target(session, |id| {
        id.starts_with("lilia.ui.navigation.") && id.contains("automation")
    })?;
    ui_click(session, &navigation)?;
    ui_click(session, "lilia.ui.automation.new")?;
    wait_observation(session, |value| {
        value
            .pointer("/observation/automationNodeCount")
            .and_then(Value::as_u64)
            == Some(1)
    })?;
    reveal_automation_control(session, "auto-name")?;
    ui_input(session, "lilia.ui.automation.auto-name", "验收工作流")?;
    reveal_automation_control(session, "auto-add-human")?;
    ui_click(session, "lilia.ui.automation.auto-add-human")?;
    wait_observation(session, |value| {
        value
            .pointer("/observation/automationNodeCount")
            .and_then(Value::as_u64)
            == Some(2)
    })?;
    reveal_automation_control(session, "auto-node-prompt")?;
    ui_input(
        session,
        "lilia.ui.automation.auto-node-prompt",
        "请确认本次发布",
    )?;
    wait_observation(session, |value| {
        value
            .pointer("/observation/automationSelectedNodeConfigDraft/prompt")
            .and_then(Value::as_str)
            == Some("请确认本次发布")
    })?;
    reveal_automation_control(session, "auto-publish")?;
    ui_click(session, "lilia.ui.automation.auto-publish")?;
    let published = wait_observation(session, |value| {
        value
            .pointer("/observation/automationPublished")
            .and_then(Value::as_bool)
            == Some(true)
    })?;
    write_json(
        &session.run_dir.join("automation-published.json"),
        &published,
    )?;
    let run = wait_matching_target(session, |id| id.ends_with(".auto-run"))?;
    ui_click(session, &run)?;
    let waiting = wait_observation(session, |value| {
        value
            .pointer("/observation/automationRunStatus")
            .and_then(Value::as_str)
            == Some("waiting_user")
    })?;
    write_json(&session.run_dir.join("automation-waiting.json"), &waiting)?;
    reveal_automation_control(session, "auto-response")?;
    ui_input(session, "lilia.ui.automation.auto-response", "确认继续")?;
    reveal_automation_control(session, "auto-resume")?;
    ui_click(session, "lilia.ui.automation.auto-resume")?;
    let completed = wait_observation(session, |value| {
        value
            .pointer("/observation/automationRunStatus")
            .and_then(Value::as_str)
            == Some("succeeded")
    })?;
    write_json(
        &session.run_dir.join("automation-completed.json"),
        &completed,
    )?;
    ui_click(session, &run)?;
    wait_observation(session, |value| {
        value
            .pointer("/observation/automationRunStatus")
            .and_then(Value::as_str)
            == Some("waiting_user")
    })?;
    reveal_automation_control(session, "auto-cancel")?;
    ui_click(session, "lilia.ui.automation.auto-cancel")?;
    let cancelled = wait_observation(session, |value| {
        value
            .pointer("/observation/automationRunStatus")
            .and_then(Value::as_str)
            == Some("cancelled")
    })?;
    write_json(
        &session.run_dir.join("automation-cancelled.json"),
        &cancelled,
    )?;
    session.capture(&session.run_dir.join("automation.png"))
}

fn extension_page(session: &Session, tab: &str) -> Result {
    let tab_target = format!("lilia.ui.settings.tab.{tab}");
    reveal_scrolled_control(session, "lilia.ui.settings.scroll", &tab_target)?;
    ui_click(session, &tab_target)?;
    wait_observation(session, |value| {
        value
            .pointer("/observation/settingsTab")
            .and_then(Value::as_str)
            == Some(tab)
    })?;
    ui_input(session, "lilia.ui.settings.extensions-search", "")
}

fn extension_editor_input(session: &Session, field: &str, value: &str) -> Result {
    let target = format!("lilia.ui.settings.{field}");
    reveal_scrolled_control(
        session,
        "lilia.ui.settings.extensions-editor-scroll",
        &target,
    )?;
    ui_input(session, &target, value)
}

fn wait_extension_dialog_closed(session: &Session) -> Result {
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    loop {
        let ui = session.request(&serde_json::json!({"command":"ui-observe"}))?;
        require_ok(&ui, "observe extension editor dismissal")?;
        if ui
            .pointer("/snapshot/targets")
            .and_then(Value::as_array)
            .is_some_and(|targets| {
                !targets
                    .iter()
                    .any(|target| target["id"] == "lilia.ui.settings.extension-editor-cancel")
            })
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(XtaskError::failure(
                "extension_dialog_still_open",
                "extension editor did not dismiss",
            ));
        }
        thread::sleep(Duration::from_millis(80));
    }
}

fn wait_extension_file(path: &Path, predicate: impl Fn(&Value) -> bool) -> Result<Value> {
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    loop {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                if predicate(&value) {
                    return Ok(value);
                }
            }
        }
        if Instant::now() >= deadline {
            return Err(XtaskError::failure(
                "extension_authority_not_updated",
                format!(
                    "UI save did not update {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
            ));
        }
        thread::sleep(Duration::from_millis(80));
    }
}

fn assert_extension_unchanged(path: &Path, before: &Value, page: &str) -> Result {
    if &read_replay_json(path)? != before {
        return Err(XtaskError::failure(
            "extension_cancel_wrote_state",
            format!("{page} cancellation changed its authoritative file"),
        ));
    }
    Ok(())
}

fn replay_extensions(session: &Session) -> Result {
    let home = session.run_dir.join("home");
    let config = home.join("config");
    let skills_path = config.join("agentkit-skills-registry.json");
    let hooks_path = config.join("agentkit-hooks.json");
    let mcp_path = config.join("agentkit-mcp-registry.json");
    let plugins_path = config.join("agentkit-plugins-registry.json");
    let mut evidence = Vec::new();

    extension_page(session, "extensions")?;
    let skill_entry = wait_matching_target(session, |id| {
        id.starts_with("lilia.ui.settings.extensions-entry-skill:user:")
            && id.ends_with("native-debug-skill")
    })?;
    ui_click(session, &skill_entry)?;
    wait_target(session, "lilia.ui.settings.skill-native-debug-skill")?;
    session.capture(&session.run_dir.join("extensions-skill-detail.png"))?;
    let skills_before = read_replay_json(&skills_path)?;
    ui_click(session, "lilia.ui.settings.skill-create-open")?;
    extension_editor_input(session, "skill_id", "native-debug-cancelled-skill")?;
    extension_editor_input(session, "skill_description", "Cancelled local skill")?;
    session.capture(&session.run_dir.join("extensions-skill-dialog.png"))?;
    ui_click(session, "lilia.ui.settings.extension-editor-cancel")?;
    wait_extension_dialog_closed(session)?;
    assert_extension_unchanged(&skills_path, &skills_before, "Skills")?;
    if home.join("skills/native-debug-cancelled-skill").exists() {
        return Err(XtaskError::failure(
            "cancelled_skill_materialized",
            "cancelled skill created a package directory",
        ));
    }
    ui_click(session, "lilia.ui.settings.skill-create-open")?;
    extension_editor_input(session, "skill_id", "native-debug-created-skill")?;
    extension_editor_input(
        session,
        "skill_description",
        "Created through the normal Native dialog",
    )?;
    ui_click(session, "lilia.ui.settings.create-skill")?;
    let skills_after = wait_extension_file(&skills_path, |value| {
        value["packages"].as_array().is_some_and(|items| {
            items
                .iter()
                .any(|item| item["skillId"] == "native-debug-created-skill")
        })
    })?;
    wait_extension_dialog_closed(session)?;
    let skill_entry = wait_matching_target(session, |id| {
        id.starts_with("lilia.ui.settings.extensions-entry-skill:user:")
            && id.ends_with("native-debug-created-skill")
    })?;
    ui_click(session, &skill_entry)?;
    wait_target(
        session,
        "lilia.ui.settings.skill-native-debug-created-skill",
    )?;
    let skill_document = fs::read_to_string(
        home.join("skills/native-debug-created-skill/SKILL.md"),
    )
    .map_err(|error| {
        XtaskError::io(
            "created_skill_missing",
            "read UI-created Skill package",
            error,
        )
    })?;
    if !skill_document.contains("Created through the normal Native dialog") {
        return Err(XtaskError::failure(
            "created_skill_description_missing",
            "created Skill did not preserve the entered description",
        ));
    }
    evidence.push(serde_json::json!({"page":"extensions","cancelUnchanged":true,"authority":skills_after,"ui":wait_ui(session)?}));

    extension_page(session, "plugin-hooks")?;
    let hook_entry = wait_matching_target(session, |id| {
        id.starts_with("lilia.ui.settings.extensions-entry-hook:native-agentkit:user:")
    })?;
    ui_click(session, &hook_entry)?;
    let hook_edit = "lilia.ui.settings.hook-edit-native-agentkit:user";
    wait_target(session, hook_edit)?;
    session.capture(&session.run_dir.join("extensions-hooks-detail.png"))?;
    let hooks_before = read_replay_json(&hooks_path)?;
    ui_click(session, hook_edit)?;
    extension_editor_input(
        session,
        "hook-native-agentkit:user-0-command",
        "native-debug-cancelled-hook",
    )?;
    session.capture(&session.run_dir.join("extensions-hooks-dialog.png"))?;
    ui_click(session, "lilia.ui.settings.extension-editor-cancel")?;
    wait_extension_dialog_closed(session)?;
    assert_extension_unchanged(&hooks_path, &hooks_before, "Hooks")?;
    ui_click(session, hook_edit)?;
    extension_editor_input(
        session,
        "hook-native-agentkit:user-0-command",
        "native-debug-saved-disabled-hook",
    )?;
    ui_click(session, "lilia.ui.settings.hook-save-native-agentkit:user")?;
    let hooks_after = wait_extension_file(&hooks_path, |value| {
        value["handlers"][0]["command"] == "native-debug-saved-disabled-hook"
    })?;
    wait_extension_dialog_closed(session)?;
    if hooks_after["enabled"] != false {
        return Err(XtaskError::failure(
            "fixture_hook_enabled",
            "editing a disabled Hook unexpectedly enabled execution",
        ));
    }
    wait_target(session, hook_edit)?;
    evidence.push(serde_json::json!({"page":"plugin-hooks","cancelUnchanged":true,"authority":hooks_after,"ui":wait_ui(session)?}));

    extension_page(session, "plugin-mcp")?;
    let mcp_entry = wait_matching_target(session, |id| {
        id.starts_with("lilia.ui.settings.extensions-entry-mcp:")
            && id.ends_with(":native-debug-invalid")
    })?;
    ui_click(session, &mcp_entry)?;
    wait_target(session, "lilia.ui.settings.mcp-edit-native-debug-invalid")?;
    session.capture(&session.run_dir.join("extensions-mcp-detail.png"))?;
    let mcp_before = read_replay_json(&mcp_path)?;
    ui_click(session, "lilia.ui.settings.mcp-new")?;
    extension_editor_input(session, "mcp-id", "native-debug-cancelled-mcp")?;
    extension_editor_input(session, "mcp-location", "native-debug-never-execute")?;
    ui_click(session, "lilia.ui.settings.extension-editor-cancel")?;
    wait_extension_dialog_closed(session)?;
    assert_extension_unchanged(&mcp_path, &mcp_before, "MCP")?;
    ui_click(session, "lilia.ui.settings.mcp-new")?;
    extension_editor_input(session, "mcp-id", "native-debug-created-mcp")?;
    extension_editor_input(session, "mcp-location", "native-debug-never-execute")?;
    reveal_scrolled_control(
        session,
        "lilia.ui.settings.extensions-editor-scroll",
        "lilia.ui.settings.mcp-enabled",
    )?;
    ui_click(session, "lilia.ui.settings.mcp-enabled")?;
    write_json(
        &session.run_dir.join("extensions-mcp-dialog-ui.json"),
        &session.request(&serde_json::json!({"command":"ui-observe"}))?,
    )?;
    session.capture(&session.run_dir.join("extensions-mcp-dialog.png"))?;
    ui_click(session, "lilia.ui.settings.mcp-save")?;
    let mcp_after = wait_extension_file(&mcp_path, |value| {
        value["servers"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["serverId"] == "native-debug-created-mcp" && item["enabled"] == false
            })
        })
    })?;
    wait_extension_dialog_closed(session)?;
    let mcp_entry = wait_matching_target(session, |id| {
        id.starts_with("lilia.ui.settings.extensions-entry-mcp:")
            && id.ends_with(":native-debug-created-mcp")
    })?;
    ui_click(session, &mcp_entry)?;
    wait_target(
        session,
        "lilia.ui.settings.mcp-edit-native-debug-created-mcp",
    )?;
    evidence.push(serde_json::json!({"page":"plugin-mcp","cancelUnchanged":true,"authority":mcp_after,"ui":wait_ui(session)?}));

    extension_page(session, "plugin-packages")?;
    let plugin_source = home.join("agent-debug-fixtures/empty-plugin");
    ui_input(
        session,
        "lilia.ui.settings.plugin-source",
        &plugin_source.display().to_string(),
    )?;
    ui_click(session, "lilia.ui.settings.plugin-install")?;
    let installed = wait_extension_file(&plugins_path, |value| {
        value["packages"].as_array().is_some_and(|items| {
            items
                .iter()
                .any(|item| item["pluginId"] == "native-debug-empty-plugin")
        })
    })?;
    if !installed["packages"].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["pluginId"] == "native-debug-empty-plugin" && item["enabled"] == false)
    }) {
        return Err(XtaskError::failure(
            "plugin_install_default_changed",
            "local plugin must be installed disabled before explicit activation",
        ));
    }
    wait_observation(session, |value| {
        value.pointer("/observation/extensionsPluginsRegistryRevision")
            == Some(&installed["revision"])
            && value.pointer("/observation/extensionsBusy") == Some(&Value::Bool(false))
    })?;
    let plugin_path = installed["packages"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|item| item["pluginId"] == "native-debug-empty-plugin")
        })
        .and_then(|item| item["path"].as_str())
        .ok_or_else(|| {
            XtaskError::failure(
                "plugin_path_missing",
                "installed local plugin has no authoritative path",
            )
        })?;
    ui_click(
        session,
        &format!("lilia.ui.settings.extensions-entry-plugin:{plugin_path}"),
    )?;
    let toggle = "lilia.ui.settings.plugin-toggle-native-debug-empty-plugin";
    wait_target(session, toggle)?;
    session.capture(&session.run_dir.join("extensions-plugin-detail.png"))?;
    ui_click(session, toggle)?;
    let enabled = wait_extension_file(&plugins_path, |value| {
        value["packages"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["pluginId"] == "native-debug-empty-plugin" && item["enabled"] == true
            })
        })
    })?;
    wait_observation(session, |value| {
        value.pointer("/observation/extensionsPluginsRegistryRevision")
            == Some(&enabled["revision"])
            && value.pointer("/observation/extensionsBusy") == Some(&Value::Bool(false))
    })?;
    ui_click(session, toggle)?;
    let disabled = wait_extension_file(&plugins_path, |value| {
        value["packages"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["pluginId"] == "native-debug-empty-plugin" && item["enabled"] == false
            })
        })
    })?;
    wait_observation(session, |value| {
        value.pointer("/observation/extensionsPluginsRegistryRevision")
            == Some(&disabled["revision"])
            && value.pointer("/observation/extensionsBusy") == Some(&Value::Bool(false))
    })?;
    evidence.push(serde_json::json!({"page":"plugin-packages","localInstall":true,
        "transitions":["installed-disabled","enabled","disabled"],
        "installedDisabledAuthority":installed,"enabledAuthority":enabled,"finalDisabledAuthority":disabled,"ui":wait_ui(session)?}));
    write_json(
        &session.run_dir.join("extensions-replay.json"),
        &Value::Array(evidence),
    )
}

fn ui_click(session: &Session, target: &str) -> Result {
    wait_target(session, target)?;
    let response =
        session.request(&serde_json::json!({"command":"ui-click", "targetId":target}))?;
    if response["ok"] != true {
        let ui = session
            .request(&serde_json::json!({"command":"ui-observe"}))
            .ok();
        let state = session
            .request(&serde_json::json!({"command":"observe"}))
            .ok();
        let _ = write_json(
            &session.run_dir.join("ui-click-failure.json"),
            &serde_json::json!({"targetId":target,"response":response,"ui":ui,"state":state}),
        );
        let _ = session.capture(&session.run_dir.join("ui-click-failure.png"));
    }
    require_ok(&response, target)
}

fn ui_input(session: &Session, target: &str, text: &str) -> Result {
    wait_target(session, target)?;
    require_ok(
        &session
            .request(&serde_json::json!({"command":"ui-input", "targetId":target,"text":text}))?,
        target,
    )
}

fn reveal_automation_control(session: &Session, control: &str) -> Result {
    let target = format!("lilia.ui.automation.{control}");
    reveal_scrolled_control(session, "lilia.ui.automation.scroll", &target)
}

// Targets retain their complete layout bounds even when only a hit-testable
// portion is exposed. Presence alone is therefore not a reveal assertion.
fn scrolled_control_delta(ui: &Value, scroll: &str, target: &str) -> Result<Option<f64>> {
    let entries = ui.pointer("/snapshot/targets").and_then(Value::as_array);
    let find =
        |id: &str| entries.and_then(|entries| entries.iter().find(|entry| entry["id"] == id));
    let Some(control) = find(target) else {
        return Ok(None);
    };
    let viewport = find(scroll).ok_or_else(|| {
        XtaskError::failure(
            "scroll_reveal_geometry_missing",
            format!("missing scroll viewport {scroll}"),
        )
    })?;
    let rect = |entry: &Value| -> Result<[f64; 4]> {
        let mut result = [0.0; 4];
        for (index, field) in ["x", "y", "width", "height"].into_iter().enumerate() {
            result[index] = entry["bounds"][field]
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    XtaskError::failure(
                        "scroll_reveal_geometry_missing",
                        format!("{} has no finite {field}", entry["id"]),
                    )
                })?;
        }
        if result[2] <= 0.0 || result[3] <= 0.0 {
            return Err(XtaskError::failure(
                "scroll_reveal_geometry_invalid",
                "empty target or viewport",
            ));
        }
        Ok(result)
    };
    let [x, y, width, height] = rect(control)?;
    let [left, top, mut viewport_width, mut viewport_height] = rect(viewport)?;
    // Scrollbar space is excluded by the authoritative viewport metrics.
    if let Some(value) = viewport
        .pointer("/scroll/metrics/viewportWidth")
        .and_then(Value::as_f64)
    {
        viewport_width = viewport_width.min(value);
    }
    if let Some(value) = viewport
        .pointer("/scroll/metrics/viewportHeight")
        .and_then(Value::as_f64)
    {
        viewport_height = viewport_height.min(value);
    }
    if width > viewport_width
        || height > viewport_height
        || x < left
        || x + width > left + viewport_width
    {
        return Err(XtaskError::failure(
            "scroll_reveal_unreachable",
            format!("{target} cannot fit the vertical viewport {scroll}"),
        ));
    }
    let delta = if y < top {
        y - top
    } else if y + height > top + viewport_height {
        y + height - top - viewport_height
    } else {
        0.0
    };
    // ui-scroll carries native Wheel delta, not a content-offset delta.
    // RuntimeInputAdapter converts pixels to ScrollOffset by negating the wheel.
    Ok(Some(-delta))
}

fn reveal_scrolled_control(session: &Session, scroll: &str, target: &str) -> Result {
    wait_target(session, scroll)?;
    let mut attempts = Vec::new();
    let mut search = std::iter::once(10_000.0).chain(std::iter::repeat_n(-240.0, 32));
    for _ in 0..40 {
        let ui = session.request(&serde_json::json!({"command":"ui-observe"}))?;
        let delta = match scrolled_control_delta(&ui, scroll, target)? {
            Some(0.0) => {
                write_json(
                    &session.run_dir.join("scroll-reveal-last.json"),
                    &serde_json::json!({"scroll":scroll,"target":target,"ui":ui,"attempts":attempts}),
                )?;
                return Ok(());
            }
            Some(delta) => delta,
            None => match search.next() {
                Some(delta) => delta,
                None => break,
            },
        };
        attempts.push(serde_json::json!({"deltaY":delta,"ui":ui}));
        let response = session.request(&serde_json::json!({"command":"ui-scroll", "targetId":scroll, "deltaY":delta.to_string()}))?;
        if response["ok"] != true {
            write_json(&session.run_dir.join("ui-failure.json"), &ui)?;
            write_json(
                &session.run_dir.join("state-failure.json"),
                &session.request(&serde_json::json!({"command":"observe"}))?,
            )?;
            session.capture(&session.run_dir.join("ui-failure.png"))?;
        }
        require_ok(&response, &format!("scroll to {target}"))?;
        thread::sleep(Duration::from_millis(80));
    }
    write_json(
        &session.run_dir.join("scroll-reveal-failure.json"),
        &serde_json::json!({"scroll":scroll,"target":target,"attempts":attempts}),
    )?;
    Err(XtaskError::failure(
        "scroll_reveal_incomplete",
        format!("{target} was not fully revealed inside {scroll}"),
    ))
}

pub(crate) fn require_ok(response: &Value, command: &str) -> Result {
    if response.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(())
    } else {
        Err(XtaskError::failure(
            "agent_debug_command_failed",
            format!("{command}: {response}"),
        ))
    }
}

/// Checks that the exported journal is a usable post-mortem log and returns how
/// many records it holds.
///
/// An export that silently produces nothing, or loses ordering, is worse than no
/// export at all: a later investigation would read the gap as "nothing
/// happened". Feature mounts are the one class of record every run must contain,
/// so their absence is what proves the pipeline broke.
fn read_journal(path: &Path) -> Result<usize> {
    let exported = fs::read_to_string(path).map_err(|error| {
        XtaskError::io(
            "journal_export_missing",
            &format!("read the exported kernel journal at {}", path.display()),
            error,
        )
    })?;
    let mut previous = 0_u64;
    let mut mounted = 0_usize;
    let mut records = 0_usize;
    for line in exported.lines().filter(|line| !line.trim().is_empty()) {
        let record: Value = serde_json::from_str(line).map_err(|error| {
            XtaskError::failure(
                "journal_export_malformed",
                format!("the exported journal holds a line that is not a record: {error}"),
            )
        })?;
        let sequence = record["sequence"].as_u64().unwrap_or_default();
        if sequence <= previous {
            return Err(XtaskError::failure(
                "journal_export_unordered",
                format!("record {sequence} does not follow {previous}"),
            ));
        }
        previous = sequence;
        records += 1;
        if record["topic"] == "feature.mounted" {
            mounted += 1;
        }
    }
    if mounted == 0 {
        return Err(XtaskError::failure(
            "journal_export_empty",
            "the exported journal records no feature mount, so the sink never received the log",
        ));
    }
    Ok(records)
}

/// A readiness timeout on its own hides the reason the desktop never came up, so
/// fold whatever the process reported on stderr into the error.
fn describe_startup_failure(error: XtaskError, run_dir: &Path) -> XtaskError {
    let Ok(log) = fs::read_to_string(run_dir.join("desktop.stderr.log")) else {
        return error;
    };
    let reason = log.trim();
    if reason.is_empty() {
        return error;
    }
    XtaskError::blocker(error.code, format!("{}: {reason}", error.message))
}

fn wait_ready(path: &Path) -> Result<String> {
    let started = Instant::now();
    loop {
        if let Ok(value) = fs::read_to_string(path) {
            let address = value.trim();
            if !address.is_empty() {
                return Ok(address.to_owned());
            }
        }
        if started.elapsed() >= READY_TIMEOUT {
            return Err(XtaskError::blocker(
                "agent_debug_ready_timeout",
                format!(
                    "desktop did not create {} within {READY_TIMEOUT:?}",
                    path.display()
                ),
            ));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn request(address: &str, request: &Value) -> Result<Value> {
    let mut stream = TcpStream::connect(address)
        .map_err(|error| XtaskError::io("agent_debug_connect_failed", address, error))?;
    stream
        .set_read_timeout(Some(RESPONSE_TIMEOUT))
        .map_err(|error| {
            XtaskError::io(
                "agent_debug_timeout_failed",
                "set debug socket timeout",
                error,
            )
        })?;
    writeln!(stream, "{request}").map_err(|error| {
        XtaskError::io("agent_debug_write_failed", "write debug request", error)
    })?;
    stream.flush().map_err(|error| {
        XtaskError::io("agent_debug_write_failed", "flush debug request", error)
    })?;
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .map_err(|error| XtaskError::io("agent_debug_read_failed", "read debug response", error))?;
    serde_json::from_str(response.trim())
        .map_err(|error| XtaskError::failure("agent_debug_response_invalid", error.to_string()))
}

fn write_json(path: &Path, value: &Value) -> Result {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| XtaskError::failure("artifact_serialize_failed", error.to_string()))?;
    fs::write(path, bytes).map_err(|error| {
        XtaskError::io("artifact_write_failed", &path.display().to_string(), error)
    })
}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn composer_digest(content: &str) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(content.as_bytes()))
}

fn validate_capture_content(image: &image::DynamicImage) -> Result {
    let pixels = image.to_rgba8();
    let margin = (pixels.width() / 50).max(1);
    let top = (pixels.height() / 10).max(40).min(pixels.height() - 1);
    let bottom = pixels.height() - (pixels.height() / 50).max(1);
    let mut first = None;
    let mut varied = false;
    let mut visible = 0_u64;
    let mut sampled = 0_u64;
    for y in top..bottom {
        for x in margin..pixels.width().saturating_sub(margin) {
            let pixel = pixels.get_pixel(x, y).0;
            visible += u64::from(pixel[3] != 0);
            sampled += 1;
            if let Some(first) = first {
                varied |= first != pixel;
            } else {
                first = Some(pixel);
            }
        }
    }
    if sampled == 0 || visible * 10 < sampled * 9 || !varied {
        return Err(XtaskError::failure(
            "capture_content_empty",
            "native client area is mostly transparent or uniform; control observations and native child views do not prove a rendered WGPU frame",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod capture_tests {
    use super::*;

    #[test]
    fn reveal_requires_complete_control_bounds_and_scrolls_the_clipped_edge() {
        let observation = |y, height| {
            serde_json::json!({"snapshot":{"targets":[
                {"id":"scroll","bounds":{"x":10,"y":100,"width":200,"height":300},
                 "scroll":{"metrics":{"viewportWidth":200,"viewportHeight":300}}},
                {"id":"toggle","bounds":{"x":20,"y":y,"width":40,"height":height}}
            ]}})
        };
        // A half-exposed toggle is already in targets but must still move upward.
        assert_eq!(
            scrolled_control_delta(&observation(390, 20), "scroll", "toggle").unwrap(),
            Some(-10.0)
        );
        assert_eq!(
            scrolled_control_delta(&observation(380, 20), "scroll", "toggle").unwrap(),
            Some(0.0)
        );
        assert_eq!(
            scrolled_control_delta(&observation(90, 20), "scroll", "toggle").unwrap(),
            Some(10.0)
        );
        assert_eq!(
            scrolled_control_delta(&observation(100, 20), "scroll", "toggle").unwrap(),
            Some(0.0)
        );
        assert_eq!(
            scrolled_control_delta(&observation(100, 20), "scroll", "absent").unwrap(),
            None
        );
        assert!(scrolled_control_delta(&observation(100, 301), "scroll", "toggle").is_err());
        let mut invalid = observation(100, 20);
        invalid["snapshot"]["targets"][1]["bounds"]["width"] = Value::Null;
        assert!(scrolled_control_delta(&invalid, "scroll", "toggle").is_err());
    }

    // An explicit, bounded native acceptance entry; never starts a window in normal tests.
    #[test]
    #[ignore = "requires an unlocked desktop and exclusive native-window access"]
    fn native_extensions_reveal_wide_themes() {
        require_interactive_desktop_session().unwrap();
        for theme in ["light", "dark"] {
            let session =
                Session::start_with_viewport("agent-debug", Some((1440, 900, theme))).unwrap();
            let visible = wait_ui(&session).unwrap();
            let target = visible
                .pointer("/snapshot/targets")
                .and_then(Value::as_array)
                .unwrap()
                .iter()
                .filter_map(|entry| entry["id"].as_str())
                .find(|id| id.starts_with("lilia.ui.navigation.") && id.contains("settings"))
                .unwrap();
            ui_click(&session, target).unwrap();
            wait_observation(&session, |value| {
                value
                    .pointer("/observation/page")
                    .and_then(Value::as_str)
                    .is_some_and(|page| page.contains("settings"))
            })
            .unwrap();
            replay_extensions(&session).unwrap();
            println!("extensions reveal {theme}: {}", session.run_dir.display());
        }
    }

    #[test]
    fn popup_capture_selects_the_new_native_identity_without_size_or_order_assumptions() {
        assert_eq!(unique_added_window(&[100, 20], &[5, 20, 100]).unwrap(), 5);
        // Closing an unrelated helper or changing enumeration order is harmless.
        assert_eq!(unique_added_window(&[100, 20], &[100, 5]).unwrap(), 5);
        assert!(unique_added_window(&[100], &[100]).is_err());
        assert!(unique_added_window(&[100], &[100, 5, 6]).is_err());
        assert!(unique_added_window(&[100], &[100, 0]).is_err());
    }

    #[test]
    fn macos_lock_state_distinguishes_console_and_user_session_locks() {
        assert!(macos_session_is_locked(
            r#""CGSSessionScreenIsLocked"=Yes,"IOConsoleLocked" = No"#
        ));
        assert!(macos_session_is_locked(r#""IOConsoleLocked" = Yes"#));
        assert!(!macos_session_is_locked(
            r#""CGSSessionScreenIsLocked"=No,"IOConsoleLocked" = No"#
        ));
    }

    #[test]
    fn capture_requires_content_beyond_native_window_chrome() {
        let mut empty = image::RgbaImage::new(200, 120);
        for x in 4..35 {
            empty.put_pixel(x, 10, image::Rgba([255, 0, 0, 255]));
        }
        assert!(validate_capture_content(&image::DynamicImage::ImageRgba8(empty)).is_err());
        let mut native_child_only = image::RgbaImage::new(200, 120);
        for y in 40..118 {
            for x in 130..198 {
                native_child_only.put_pixel(
                    x,
                    y,
                    image::Rgba([100 + (y % 20) as u8, 140, 160, 255]),
                );
            }
        }
        assert!(
            validate_capture_content(&image::DynamicImage::ImageRgba8(native_child_only)).is_err()
        );
        let blank = image::RgbaImage::from_pixel(200, 120, image::Rgba([30, 30, 30, 255]));
        assert!(validate_capture_content(&image::DynamicImage::ImageRgba8(blank.clone())).is_err());
        let mut content = blank;
        for y in 60..100 {
            for x in 50..150 {
                content.put_pixel(x, y, image::Rgba([180, 180, 180, 255]));
            }
        }
        assert!(validate_capture_content(&image::DynamicImage::ImageRgba8(content)).is_ok());
    }
}
