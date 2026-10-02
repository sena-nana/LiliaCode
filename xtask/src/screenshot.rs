use std::fs;
use std::path::{Path, PathBuf};

use crate::{repo_root, Result, XtaskError};

pub fn run(arguments: &[String]) -> Result {
    if arguments == ["--matrix"] {
        for (width, height) in [(960, 600), (1440, 900)] {
            for theme in ["light", "dark"] {
                let output = repo_root()?.join(format!(
                    "target/offscreen-screenshots/product-{width}x{height}-{theme}.png"
                ));
                render_product(&output, theme, width, height)?;
                println!("screenshot: ok ({})", output.display());
            }
        }
        return Ok(());
    }
    let output = parse_output(arguments)?;
    let output = match output {
        Some(path) if path.is_absolute() => path,
        Some(path) => repo_root()?.join(path),
        None => repo_root()?.join("target/offscreen-screenshots/product.png"),
    };
    render_product(&output, "light", 1180, 760)?;
    println!("screenshot: ok ({})", output.display());
    Ok(())
}

fn render_product(output: &Path, theme: &str, width: u32, height: u32) -> Result {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            XtaskError::io(
                "screenshot_directory_failed",
                "create screenshot directory",
                error,
            )
        })?;
    }
    let root = repo_root()?;
    let mut command = crate::command("cargo");
    command
        .current_dir(&root)
        .env("LILIA_OFFSCREEN_OUTPUT", output)
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
    if !output.is_file() || output.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(XtaskError::failure(
            "screenshot_missing",
            format!("offscreen renderer did not create {}", output.display()),
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
        [flag, value] if flag == "--out" => Ok(Some(PathBuf::from(value))),
        _ => Err(usage()),
    }
}

fn usage() -> XtaskError {
    XtaskError::failure(
        "usage",
        "usage: cargo xtask screenshot [--out <png> | --matrix]",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_output_path_and_rejects_anything_else() {
        assert_eq!(parse_output(&[]).unwrap(), None);
        assert_eq!(
            parse_output(&["--out".to_owned(), "artifacts/shot.png".to_owned()]).unwrap(),
            Some(PathBuf::from("artifacts/shot.png"))
        );
        assert_eq!(
            parse_output(&["--out".to_owned()]).unwrap_err().code,
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
