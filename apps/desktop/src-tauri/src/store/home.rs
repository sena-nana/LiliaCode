use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const LILIA_HOME_ENV: &str = "LILIA_HOME";
const REDIRECT_FILE: &str = ".redirect";
const DEFAULT_HOME_SUBDIR: &str = ".lilia";

pub fn resolve_lilia_home() -> PathBuf {
    let primary = resolve_home_candidate();
    let home = primary.unwrap_or_else(|| PathBuf::from(".lilia"));
    let _ = ensure_layout(&home);
    home
}

fn resolve_home_candidate() -> Option<PathBuf> {
    if let Ok(env_val) = env::var(LILIA_HOME_ENV) {
        let trimmed = env_val.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }

    let default_home = dirs::home_dir().map(|d| d.join(DEFAULT_HOME_SUBDIR));

    if let Some(home) = &default_home {
        let redirect = home.join(REDIRECT_FILE);
        if redirect.is_file() {
            if let Ok(raw) = fs::read_to_string(&redirect) {
                let target = raw.trim();
                if !target.is_empty() {
                    return Some(PathBuf::from(target));
                }
            }
        }
    }

    default_home
}

pub(super) fn ensure_layout(home: &Path) -> std::io::Result<()> {
    fs::create_dir_all(home)?;
    for sub in ["config", "db", "cache"] {
        fs::create_dir_all(home.join(sub))?;
    }
    Ok(())
}
