use super::{capture_window, require_ok, write_json, Session};
use crate::{Result, XtaskError};
use serde_json::json;
use std::time::{Duration, Instant};

pub(super) fn run(session: &Session, theme: &str) -> Result<()> {
    for tab in [
        "project",
        "provider",
        "agent",
        "quota",
        "extensions",
        "remote",
        "desktop",
        "data",
        "about",
        "appearance",
    ] {
        require_ok(
            &session
                .request(&json!({"command":"click", "targetId":format!("lilia.settings.{tab}")}))?,
            "select settings page",
        )?;
        let deadline = Instant::now() + Duration::from_secs(15);
        let observation = loop {
            let state = session.request(&json!({"command":"observe"}))?;
            require_ok(&state, "observe settings page")?;
            if state
                .pointer("/observation/settingsTab")
                .and_then(|value| value.as_str())
                == Some(tab)
                && state
                    .pointer("/observation/theme")
                    .and_then(|value| value.as_str())
                    == Some(theme)
            {
                break state;
            }
            if Instant::now() >= deadline {
                return Err(XtaskError::failure(
                    "settings_page_timeout",
                    format!("settings did not reach {tab} in {theme} theme"),
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        if observation
            .to_string()
            .contains("sk-native-agent-debug-fixture")
        {
            return Err(XtaskError::failure(
                "settings_secret_leak",
                "settings observation contains the secret canary",
            ));
        }
        write_json(
            &session.run_dir.join(format!("settings-{tab}-{theme}.json")),
            &observation,
        )?;
        capture_window(
            session.pid(),
            &session.run_dir.join(format!("settings-{tab}-{theme}.png")),
        )?;
    }
    Ok(())
}
