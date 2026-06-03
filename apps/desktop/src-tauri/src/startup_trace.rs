use std::env;
use std::sync::OnceLock;
use std::time::Instant;

static STARTUP_START: OnceLock<Instant> = OnceLock::new();

fn enabled() -> bool {
    cfg!(debug_assertions) || env::var("LILIA_STARTUP_TRACE").is_ok_and(|value| value == "1")
}

pub(crate) fn init() {
    let _ = STARTUP_START.set(Instant::now());
    mark("run start");
}

pub(crate) fn mark(label: &str) {
    if !enabled() {
        return;
    }
    let start = STARTUP_START.get_or_init(Instant::now);
    eprintln!("[startup] +{}ms {label}", start.elapsed().as_millis());
}

#[tauri::command]
pub(crate) fn startup_trace_frontend_mark(label: String, frontend_elapsed_ms: u64) {
    mark(&format!("frontend +{frontend_elapsed_ms}ms {label}"));
}
