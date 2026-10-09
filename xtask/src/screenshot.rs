use std::fs;
use std::path::{Path, PathBuf};

use crate::{repo_root, Result, XtaskError};

pub fn run(arguments: &[String]) -> Result {
    let matrix = arguments.first().is_some_and(|flag| flag == "--matrix");
    let rest = if matrix { &arguments[1..] } else { arguments };
    let dir = match parse_output(rest)? {
        Some(path) if path.is_absolute() => path,
        Some(path) => repo_root()?.join(path),
        None => repo_root()?.join("target/offscreen-screenshots"),
    };
    if matrix {
        for (width, height) in [(960, 600), (1440, 900)] {
            for theme in ["light", "dark"] {
                render_product(&dir, theme, width, height)?;
            }
        }
    } else {
        render_product(&dir, "light", 1180, 760)?;
    }
    println!("screenshot: ok ({})", dir.display());
    Ok(())
}

fn render_product(dir: &Path, theme: &str, width: u32, height: u32) -> Result {
    fs::create_dir_all(dir).map_err(|error| {
        XtaskError::io(
            "screenshot_directory_failed",
            "create screenshot directory",
            error,
        )
    })?;
    let root = repo_root()?;
    let mut command = crate::command("cargo");
    command
        .current_dir(&root)
        .env("LILIA_OFFSCREEN_DIR", dir)
        .env("LILIA_OFFSCREEN_THEME", theme)
        .env("LILIA_OFFSCREEN_WIDTH", width.to_string())
        .env("LILIA_OFFSCREEN_HEIGHT", height.to_string())
        .args([
            "test",
            "--locked",
            "-p",
            "lilia-desktop",
            "product_shell_paints_offscreen",
            "--lib",
            "--",
            "--nocapture",
        ]);
    configure_headless_gpu(&mut command);
    crate::run(&mut command, "render product shell offscreen")?;
    let expected = dir.join(format!("conversation-{width}x{height}-{theme}.png"));
    if !expected.is_file() || expected.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(XtaskError::failure(
            "screenshot_missing",
            format!("offscreen renderer did not create {}", expected.display()),
        ));
    }
    Ok(())
}

/// llvmpipe's GL adapter cannot render NanaUI's optional motion-evaluation
/// target (`Rgba32Float`). Chromium's bundled SwiftShader Vulkan adapter can,
/// and remains fully headless, so prefer it when this Linux environment ships
/// the ICD. Existing caller overrides are respected.
fn configure_headless_gpu(command: &mut std::process::Command) {
    if !cfg!(target_os = "linux") || std::env::var_os("WGPU_BACKEND").is_some() {
        return;
    }
    if std::env::var_os("VK_ICD_FILENAMES").is_some() {
        command.env("WGPU_BACKEND", "vulkan");
        return;
    }
    for candidate in [
        "/usr/lib/chromium/vk_swiftshader_icd.json",
        "/usr/lib/chromium-browser/vk_swiftshader_icd.json",
    ] {
        if Path::new(candidate).is_file() {
            command
                .env("WGPU_BACKEND", "vulkan")
                .env("VK_ICD_FILENAMES", candidate);
            return;
        }
    }
}

fn parse_output(arguments: &[String]) -> Result<Option<PathBuf>> {
    match arguments {
        [] => Ok(None),
        [flag, value] if flag == "--out-dir" => Ok(Some(PathBuf::from(value))),
        _ => Err(usage()),
    }
}

fn usage() -> XtaskError {
    XtaskError::failure(
        "usage",
        "usage: cargo xtask screenshot [--matrix] [--out-dir <dir>]",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_output_path_and_rejects_anything_else() {
        assert_eq!(parse_output(&[]).unwrap(), None);
        assert_eq!(
            parse_output(&["--out-dir".to_owned(), "artifacts/shots".to_owned()]).unwrap(),
            Some(PathBuf::from("artifacts/shots"))
        );
        assert_eq!(
            parse_output(&["--out-dir".to_owned()]).unwrap_err().code,
            "usage"
        );
        assert_eq!(
            parse_output(&["--nope".to_owned(), "1".to_owned()])
                .unwrap_err()
                .code,
            "usage"
        );
    }
}
