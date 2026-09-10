use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

use crate::agent_debug::{require_ok, Session};
use crate::{output, Result, XtaskError};

const SAMPLES: usize = 30;
const PROCESS_STARTUP_SAMPLES: usize = 5;
const EXPECTED_TIMELINE_EVENTS: u64 = 1_000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema_version: u32,
    platform: &'static str,
    historical_baseline: &'static str,
    samples: usize,
    process_startup_ready_ms: Vec<f64>,
    process_startup_ready_p95_ms: f64,
    startup_runs: Vec<Value>,
    composer_frame_ms: Vec<f64>,
    panel_resize_frame_ms: Vec<f64>,
    composer_frame_p95_ms: f64,
    panel_resize_frame_p95_ms: f64,
    thousand_timeline_ready_ms: f64,
    idle_cpu_percent: f64,
    idle_rss_bytes: u64,
    idle_process_sampling: IdleProcessSampling,
    timeline_event_count: u64,
    measurement_contract: Value,
    gates: Gates,
    passed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Gates {
    process_startup_ready_p95_ms: f64,
    frame_p95_ms: f64,
    thousand_timeline_ms: f64,
    idle_cpu_percent: f64,
    idle_rss_bytes: u64,
}

#[derive(Debug, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProcessSample {
    cpu_seconds: f64,
    working_set_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IdleProcessSampling {
    pid: u32,
    before: ProcessSample,
    after: ProcessSample,
    elapsed_seconds: f64,
    logical_processors: usize,
}

impl IdleProcessSampling {
    fn cpu_percent(&self) -> Result<f64> {
        let delta = self.after.cpu_seconds - self.before.cpu_seconds;
        if !self.elapsed_seconds.is_finite()
            || self.elapsed_seconds <= 0.0
            || self.logical_processors == 0
            || !delta.is_finite()
            || delta < 0.0
        {
            return Err(XtaskError::failure(
                "performance_process_sample_invalid",
                "CPU samples require a positive measured interval, processor count and monotonic counter",
            ));
        }
        Ok(delta * 100.0 / self.elapsed_seconds / self.logical_processors as f64)
    }
}

pub fn run() -> Result {
    if !cfg!(any(target_os = "windows", target_os = "macos")) {
        return Err(XtaskError::blocker(
            "desktop_required",
            "native performance requires macOS or Windows with a real WGPU desktop",
        ));
    }
    crate::agent_debug::require_interactive_desktop_session()?;
    let gates = Gates {
        // Preserve the existing override name and absolute threshold for compatibility.
        process_startup_ready_p95_ms: env_f64("LILIA_PERFORMANCE_COLD_START_P95_MS", 15_000.0),
        frame_p95_ms: env_f64("LILIA_PERFORMANCE_FRAME_P95_MS", 100.0),
        thousand_timeline_ms: env_f64("LILIA_PERFORMANCE_TIMELINE_MS", 30_000.0),
        idle_cpu_percent: env_f64("LILIA_PERFORMANCE_IDLE_CPU_PERCENT", 10.0),
        idle_rss_bytes: env_u64("LILIA_PERFORMANCE_IDLE_RSS_BYTES", 1_073_741_824),
    };

    let session = Session::start("performance")?;
    let timeline_started = Instant::now();
    act(
        &session,
        serde_json::json!({
            "command": "click",
            "targetId": "lilia.project.equivalence-performance-v1-project"
        }),
    )?;
    act(
        &session,
        serde_json::json!({
            "command": "click",
            "targetId": "lilia.task.equivalence-performance-v1-task"
        }),
    )?;
    let mut observation = observe(&session)?;
    while observation
        .pointer("/observation/timelineHasMoreBefore")
        .and_then(Value::as_bool)
        == Some(true)
    {
        observation = act(
            &session,
            serde_json::json!({
                "command": "click",
                "targetId": "lilia.task-session.timeline.load-earlier"
            }),
        )?;
    }
    let timeline_count = observation
        .pointer("/observation/timelineEventCount")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    if timeline_count != EXPECTED_TIMELINE_EVENTS {
        return Err(XtaskError::failure(
            "performance_corpus_incomplete",
            format!("loaded {timeline_count}/{EXPECTED_TIMELINE_EVENTS} timeline events"),
        ));
    }
    let thousand_timeline_ready_ms = timeline_started.elapsed().as_secs_f64() * 1_000.0;
    crate::agent_debug::wait_target(&session, "lilia.task-session.composer.input")?;

    let mut composer = Vec::with_capacity(SAMPLES);
    for index in 0..SAMPLES {
        let response = act(
            &session,
            serde_json::json!({
                "command": "ui-input-frame",
                "targetId": "lilia.task-session.composer.input",
                "text": format!("固定性能输入 {index}")
            }),
        )?;
        composer.push(duration_ms(&response, "composer input-frame")?);
    }
    if !visible(&observation, "lilia.task-session.inspector") {
        observation = act(
            &session,
            serde_json::json!({
                "command": "click",
                "targetId": "lilia.task-session.inspector.toggle"
            }),
        )?;
    }
    if !visible(&observation, "lilia.task-session.inspector") {
        return Err(XtaskError::failure(
            "performance_inspector_missing",
            "inspector did not become visible before resize measurement",
        ));
    }
    let mut resize = Vec::with_capacity(SAMPLES);
    for index in 0..SAMPLES {
        let response = act(
            &session,
            serde_json::json!({
                "command": "resize-panel-frame",
                // 调试协议的字段一律是字符串，数字会在边界解析处被拒。
                "extent": if index % 2 == 0 { "368" } else { "352" }
            }),
        )?;
        resize.push(duration_ms(&response, "panel resize-frame")?);
    }

    crate::agent_debug::capture_window(session.pid(), &session.run_dir.join("performance.png"))?;
    thread::sleep(Duration::from_secs(5));
    let before = process_sample(session.pid())?;
    let sampling_started = Instant::now();
    thread::sleep(Duration::from_secs(1));
    let after = process_sample(session.pid())?;
    let elapsed_seconds = sampling_started.elapsed().as_secs_f64();
    let processors = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    let idle_rss_bytes = after.working_set_bytes;
    let idle_process_sampling = IdleProcessSampling {
        pid: session.pid(),
        before,
        after,
        elapsed_seconds,
        logical_processors: processors,
    };
    let idle_cpu_percent = idle_process_sampling.cpu_percent()?;

    session.verify_runtime_artifact()?;
    let mut process_startup_ready_ms = vec![session.startup_ms];
    let mut startup_runs = vec![startup_evidence(&session)?];
    let run_dir = session.run_dir.clone();
    // Each measured process uses the same seeded performance profile. Close the
    // first process before subsequent launches so it does not remain a background load.
    drop(session);
    for _ in 1..PROCESS_STARTUP_SAMPLES {
        let startup = Session::start("performance")?;
        startup.verify_runtime_artifact()?;
        process_startup_ready_ms.push(startup.startup_ms);
        startup_runs.push(startup_evidence(&startup)?);
    }
    let process_startup_ready_p95_ms = percentile(&process_startup_ready_ms, 0.95)?;
    let composer_frame_p95_ms = percentile(&composer, 0.95)?;
    let panel_resize_frame_p95_ms = percentile(&resize, 0.95)?;
    let passed = process_startup_ready_p95_ms <= gates.process_startup_ready_p95_ms
        && composer_frame_p95_ms <= gates.frame_p95_ms
        && panel_resize_frame_p95_ms <= gates.frame_p95_ms
        && thousand_timeline_ready_ms <= gates.thousand_timeline_ms
        && idle_cpu_percent <= gates.idle_cpu_percent
        && idle_rss_bytes <= gates.idle_rss_bytes;
    let report = Report {
        schema_version: 2,
        platform: std::env::consts::OS,
        historical_baseline: "not_compared: matching historical platform corpus unavailable",
        samples: SAMPLES,
        process_startup_ready_ms,
        process_startup_ready_p95_ms,
        startup_runs,
        composer_frame_ms: composer,
        panel_resize_frame_ms: resize,
        composer_frame_p95_ms,
        panel_resize_frame_p95_ms,
        thousand_timeline_ready_ms,
        idle_cpu_percent,
        idle_rss_bytes,
        idle_process_sampling,
        timeline_event_count: timeline_count,
        measurement_contract: serde_json::json!({
            "startup": "process spawn to debug ready file; same performance-v1 fixture in five isolated homes; excludes build, seeding and artifact verification; not OS-cache-cold or content-first-present latency",
            "frame": "handler start to primary window_frame_presented; 30 serial operations per category; excludes request transport and dispatch wait; not continuous FPS",
            "percentile": "sorted[ceil((n - 1) * 0.95)]; raw arrays retain acquisition order",
            "timeline": "project/task selection and pagination until observation reports 1000 loaded events; excludes seeding and startup; not 1000 simultaneous painted rows",
            "cpu": "main-process cumulative CPU seconds delta / measured sample-completion interval / available_parallelism * 100; interval includes second process-query latency",
            "rss": "single after-sample main-process resident memory; macOS ps KiB converted to bytes; excludes child processes and GPU memory; not peak RSS",
            "frameRunIndex": 0,
            "fixture": "tests/desktop/performance-v1.json"
        }),
        gates,
        passed,
    };
    let path = run_dir.join("performance.json");
    fs::write(&path, serde_json::to_vec_pretty(&report).unwrap()).map_err(|error| {
        XtaskError::io(
            "performance_artifact_failed",
            &path.display().to_string(),
            error,
        )
    })?;
    if !passed {
        return Err(XtaskError::failure(
            "performance_gate_failed",
            format!(
                "one or more native absolute gates failed; artifact: {}",
                path.display()
            ),
        ));
    }
    println!("performance: ok ({})", path.display());
    Ok(())
}

fn startup_evidence(session: &Session) -> Result<Value> {
    let read = |name: &str| -> Result<Value> {
        let path = session.run_dir.join(name);
        let bytes = fs::read(&path).map_err(|error| {
            XtaskError::io(
                "performance_evidence_failed",
                &path.display().to_string(),
                error,
            )
        })?;
        serde_json::from_slice(&bytes)
            .map_err(|error| XtaskError::failure("performance_evidence_invalid", error.to_string()))
    };
    Ok(serde_json::json!({
        "runDirectory": session.run_dir,
        "pid": session.pid(),
        "source": read("source.json")?,
        "buildArtifact": read("build-artifact.json")?,
        "runtimeArtifactVerification": read("runtime-artifact-verified.json")?,
    }))
}

fn observe(session: &Session) -> Result<Value> {
    act(session, serde_json::json!({ "command": "observe" }))
}

fn act(session: &Session, request: Value) -> Result<Value> {
    let command = request
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let response = session.request(&request)?;
    if let Err(error) = require_ok(&response, command) {
        let state = session
            .request(&serde_json::json!({"command":"ui-observe"}))
            .ok();
        let _ = std::fs::write(
            session.run_dir.join("ui-failure.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"request":request,"response":response,"ui":state}),
            )
            .unwrap_or_default(),
        );
        let _ = crate::agent_debug::capture_window(
            session.pid(),
            &session.run_dir.join("ui-failure.png"),
        );
        return Err(error);
    }
    Ok(response)
}

fn visible(response: &Value, target: &str) -> bool {
    response
        .pointer("/observation/visibleTargetIds")
        .and_then(Value::as_array)
        .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(target)))
}

fn duration_ms(response: &Value, label: &str) -> Result<f64> {
    response
        .get("durationMs")
        .and_then(Value::as_f64)
        .filter(|duration| duration.is_finite() && *duration >= 0.0)
        .ok_or_else(|| XtaskError::failure("performance_duration_missing", label))
}

fn process_sample(pid: u32) -> Result<ProcessSample> {
    if cfg!(target_os = "macos") {
        let value = output(
            crate::command("/bin/ps").args(["-p", &pid.to_string(), "-o", "time=", "-o", "rss="]),
            "sample native desktop process",
        )?;
        return parse_macos_process_sample(&value);
    }
    let script = format!(
        "$p=Get-Process -Id {pid}; @{{cpuSeconds=[double]$p.CPU;workingSetBytes=[uint64]$p.WorkingSet64}} | ConvertTo-Json -Compress"
    );
    let value = output(
        crate::command("powershell.exe").args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ]),
        "sample native desktop process",
    )?;
    serde_json::from_str(value.trim()).map_err(|error| {
        XtaskError::failure("performance_process_sample_invalid", error.to_string())
    })
}

fn parse_macos_process_sample(value: &str) -> Result<ProcessSample> {
    let invalid = || {
        XtaskError::failure(
            "performance_process_sample_invalid",
            "ps did not return CPU time and resident memory",
        )
    };
    let mut fields = value.split_whitespace();
    let time = fields.next().ok_or_else(invalid)?;
    let mut seconds = 0.0;
    for part in time.split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().map_err(|_| invalid())?;
    }
    let kib = fields
        .next()
        .ok_or_else(invalid)?
        .parse::<u64>()
        .map_err(|_| invalid())?;
    if !seconds.is_finite() || seconds < 0.0 || fields.next().is_some() {
        return Err(invalid());
    }
    Ok(ProcessSample {
        cpu_seconds: seconds,
        working_set_bytes: kib.checked_mul(1024).ok_or_else(invalid)?,
    })
}

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn percentile(samples: &[f64], quantile: f64) -> Result<f64> {
    if samples.is_empty()
        || !(0.0..=1.0).contains(&quantile)
        || samples
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(XtaskError::failure(
            "performance_samples_invalid",
            "percentile requires samples and a quantile between zero and one",
        ));
    }
    let mut samples = samples.to_vec();
    samples.sort_by(f64::total_cmp);
    let index = ((samples.len() - 1) as f64 * quantile).ceil() as usize;
    Ok(samples[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_cpu_sample_converts_accumulated_time_and_kib() {
        let sample = parse_macos_process_sample(" 1:02.50  1024\n").unwrap();
        assert_eq!(sample.cpu_seconds, 62.5);
        assert_eq!(sample.working_set_bytes, 1_048_576);
        assert!(parse_macos_process_sample("not-running").is_err());
    }

    #[test]
    fn percentile_uses_nearest_rank_without_hiding_tail_latency() {
        let samples = (1..=20).rev().map(f64::from).collect::<Vec<_>>();
        assert_eq!(percentile(&samples, 0.95).unwrap(), 20.0);
        assert_eq!(samples.first(), Some(&20.0));
        assert!(percentile(&[f64::NAN], 0.95).is_err());
        assert!(percentile(&[f64::INFINITY], 0.95).is_err());
        assert!(percentile(&[-1.0], 0.95).is_err());
    }

    #[test]
    fn cpu_rate_uses_measured_interval_and_machine_capacity() {
        let mut sampling = IdleProcessSampling {
            pid: 1,
            before: ProcessSample {
                cpu_seconds: 10.0,
                working_set_bytes: 1024,
            },
            after: ProcessSample {
                cpu_seconds: 11.0,
                working_set_bytes: 2048,
            },
            elapsed_seconds: 2.0,
            logical_processors: 4,
        };
        assert_eq!(sampling.cpu_percent().unwrap(), 12.5);
        sampling.elapsed_seconds = 0.5;
        assert_eq!(sampling.cpu_percent().unwrap(), 50.0);
        sampling.elapsed_seconds = 0.0;
        assert!(sampling.cpu_percent().is_err());
        sampling.elapsed_seconds = 1.0;
        sampling.after.cpu_seconds = 9.0;
        assert!(sampling.cpu_percent().is_err());
        sampling.after.cpu_seconds = 11.0;
        sampling.logical_processors = 0;
        assert!(sampling.cpu_percent().is_err());
        sampling.logical_processors = 4;
        sampling.elapsed_seconds = f64::NAN;
        assert!(sampling.cpu_percent().is_err());
    }

    #[test]
    fn frame_duration_rejects_invalid_measurements() {
        assert_eq!(
            duration_ms(&serde_json::json!({"durationMs": 0.0}), "frame").unwrap(),
            0.0
        );
        assert!(duration_ms(&serde_json::json!({"durationMs": -1.0}), "frame").is_err());
        assert!(duration_ms(&serde_json::json!({"durationMs": null}), "frame").is_err());
        assert!(duration_ms(&serde_json::json!({}), "frame").is_err());
    }

    #[test]
    fn saved_raw_samples_reproduce_frame_percentile_in_acquisition_order() {
        let samples = (1..=30).rev().map(f64::from).collect::<Vec<_>>();
        let encoded = serde_json::to_vec(&samples).unwrap();
        let saved: Vec<f64> = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(saved, samples);
        assert_eq!(percentile(&saved, 0.95).unwrap(), 29.0);
        assert_eq!(percentile(&[1.0, 5.0, 3.0, 2.0, 4.0], 0.95).unwrap(), 5.0);
    }
}
