use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{Result, XtaskError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceFingerprint {
    workspace_path: PathBuf,
    source_sha256: String,
    source_file_count: usize,
}

pub(super) fn local_nana_path(root: &Path) -> Result<Option<PathBuf>> {
    resolve_local_nana_path(&mut metadata_command(root))
}

fn metadata_command(root: &Path) -> Command {
    let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command
        .current_dir(root)
        .args(["metadata", "--locked", "--format-version", "1"]);
    command
}

fn resolve_local_nana_path(command: &mut Command) -> Result<Option<PathBuf>> {
    let output = command.output().map_err(|error| {
        XtaskError::io(
            "source_metadata_failed",
            "resolve actual NanaUI dependency",
            error,
        )
    })?;
    if !output.status.success() {
        return Err(XtaskError::failure(
            "source_metadata_failed",
            format!(
                "Cargo metadata failed ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        XtaskError::failure(
            "source_metadata_invalid",
            format!("parse Cargo metadata: {error}"),
        )
    })?;
    select_local_nana_path(&metadata)
}

fn select_local_nana_path(metadata: &serde_json::Value) -> Result<Option<PathBuf>> {
    let invalid = || {
        XtaskError::failure(
            "source_metadata_invalid",
            "Cargo metadata must include packages and the full resolved dependency graph",
        )
    };
    let packages = metadata["packages"].as_array().ok_or_else(invalid)?;
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or_else(invalid)?;
    let resolved = nodes
        .iter()
        .map(|node| node["id"].as_str().ok_or_else(invalid))
        .collect::<Result<std::collections::HashSet<_>>>()?;
    let mut candidates = packages.iter().filter(|package| {
        package["name"] == "nana-ui"
            && package["id"]
                .as_str()
                .is_some_and(|id| resolved.contains(id))
    });
    let package = candidates.next().ok_or_else(|| {
        XtaskError::failure(
            "source_dependency_missing",
            "the resolved Cargo dependency graph contains no nana-ui package",
        )
    })?;
    if candidates.next().is_some() {
        return Err(XtaskError::failure(
            "source_dependency_ambiguous",
            "the resolved Cargo dependency graph contains multiple nana-ui packages",
        ));
    }
    let source = package.get("source").ok_or_else(invalid)?;
    match source {
        serde_json::Value::String(source) if !source.is_empty() => Ok(None),
        serde_json::Value::Null => {
            let manifest = package["manifest_path"]
                .as_str()
                .map(Path::new)
                .ok_or_else(invalid)?;
            if !manifest.is_absolute()
                || manifest.file_name().is_none_or(|name| name != "Cargo.toml")
            {
                return Err(invalid());
            }
            let directory = manifest.parent().ok_or_else(invalid)?;
            Ok(Some(fs::canonicalize(directory).map_err(|error| {
                XtaskError::io(
                    "source_fingerprint_failed",
                    "resolve Cargo-selected local NanaUI package",
                    error,
                )
            })?))
        }
        _ => Err(invalid()),
    }
}

pub(super) fn fingerprint(package_path: &Path) -> Result<SourceFingerprint> {
    let package_path = fs::canonicalize(package_path).map_err(|error| {
        XtaskError::io(
            "source_fingerprint_failed",
            "resolve local NanaUI patch",
            error,
        )
    })?;
    let mut workspace = None;
    for ancestor in package_path.ancestors() {
        let manifest = ancestor.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let source = fs::read_to_string(&manifest).map_err(|error| {
            XtaskError::io(
                "source_fingerprint_failed",
                "read workspace manifest",
                error,
            )
        })?;
        let value: toml::Value = toml::from_str(&source).map_err(|error| {
            XtaskError::failure(
                "source_fingerprint_failed",
                format!("parse {}: {error}", manifest.display()),
            )
        })?;
        if value.get("workspace").is_some_and(toml::Value::is_table) {
            workspace = Some(ancestor.to_owned());
            break;
        }
    }
    let workspace = workspace.ok_or_else(|| {
        XtaskError::failure(
            "source_fingerprint_failed",
            "local NanaUI patch has no Cargo workspace ancestor",
        )
    })?;
    let mut paths = vec![workspace.join("Cargo.toml")];
    if workspace.join("Cargo.lock").is_file() {
        paths.push(workspace.join("Cargo.lock"));
    }
    let crates = workspace.join("crates");
    for entry in walkdir::WalkDir::new(&crates)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry.file_name() != "target"
                && !entry.file_name().to_string_lossy().starts_with('.')
                && !(entry.file_type().is_dir() && entry.file_name() == "node_modules")
        })
    {
        let entry = entry.map_err(|error| {
            XtaskError::failure(
                "source_fingerprint_failed",
                format!("walk NanaUI crate sources: {error}"),
            )
        })?;
        if entry.file_type().is_symlink() {
            return Err(XtaskError::failure(
                "source_fingerprint_failed",
                format!(
                    "source fingerprint cannot follow symlink {}",
                    entry.path().display()
                ),
            ));
        }
        if entry.file_type().is_file() {
            paths.push(entry.into_path());
        }
    }
    paths.sort_by(|left, right| {
        left.strip_prefix(&workspace)
            .unwrap()
            .cmp(right.strip_prefix(&workspace).unwrap())
    });
    let mut digest = Sha256::new();
    for path in &paths {
        let relative = path.strip_prefix(&workspace).expect("workspace source");
        let relative = relative
            .to_str()
            .ok_or_else(|| {
                XtaskError::failure("source_fingerprint_failed", "source path is not UTF-8")
            })?
            .replace('\\', "/");
        let mut input = fs::File::open(path).map_err(|error| {
            XtaskError::io(
                "source_fingerprint_failed",
                "open NanaUI source file",
                error,
            )
        })?;
        let mut file_digest = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let size = input.read(&mut buffer).map_err(|error| {
                XtaskError::io(
                    "source_fingerprint_failed",
                    "hash NanaUI source file",
                    error,
                )
            })?;
            if size == 0 {
                break;
            }
            file_digest.update(&buffer[..size]);
        }
        digest.update((relative.len() as u64).to_be_bytes());
        digest.update(relative.as_bytes());
        digest.update(file_digest.finalize());
    }
    Ok(SourceFingerprint {
        workspace_path: workspace,
        source_sha256: format!("{:x}", digest.finalize()),
        source_file_count: paths.len(),
    })
}

pub(super) fn verify_unchanged(
    before: &Option<SourceFingerprint>,
    after: &Option<SourceFingerprint>,
) -> Result {
    if before != after {
        return Err(XtaskError::failure(
            "source_changed_during_build",
            "local NanaUI source changed during Cargo build; this executable cannot be accepted as evidence for the recorded source fingerprint",
        ));
    }
    Ok(())
}

pub(super) fn file_sha256(path: &Path) -> Result<String> {
    let mut input = fs::File::open(path)
        .map_err(|error| XtaskError::io("artifact_hash_failed", "open build artifact", error))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let size = input.read(&mut buffer).map_err(|error| {
            XtaskError::io("artifact_hash_failed", "hash build artifact", error)
        })?;
        if size == 0 {
            break;
        }
        digest.update(&buffer[..size]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn loaded_host_path(binary: &Path) -> Result<PathBuf> {
    let binary = fs::canonicalize(binary).map_err(|error| {
        XtaskError::io(
            "desktop_artifact_missing",
            "resolve desktop launcher",
            error,
        )
    })?;
    let parent = binary.parent().ok_or_else(|| {
        XtaskError::failure("desktop_artifact_missing", "launcher has no directory")
    })?;
    Ok(parent.join(format!(
        "{}liliacode_host{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    )))
}

pub(super) fn record_host_artifact(launcher: &mut serde_json::Value, cargo_output: &str) -> Result {
    let binary = launcher["executable"].as_str().ok_or_else(|| {
        XtaskError::failure(
            "desktop_artifact_missing",
            "Cargo launcher executable is missing",
        )
    })?;
    let loaded = loaded_host_path(Path::new(binary))?;
    let loaded_real = fs::canonicalize(&loaded).map_err(|error| {
        XtaskError::io(
            "desktop_host_artifact_missing",
            "resolve launcher-adjacent host library",
            error,
        )
    })?;
    let host = cargo_output
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|artifact| {
            artifact["reason"] == "compiler-artifact"
                && artifact["target"]["name"] == "liliacode_host"
                && artifact["package_id"] == launcher["package_id"]
                && artifact["target"]["crate_types"]
                    .as_array()
                    .is_some_and(|types| types.iter().any(|kind| kind == "cdylib"))
                && artifact["filenames"].as_array().is_some_and(|files| {
                    files
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .any(|file| fs::canonicalize(file).is_ok_and(|path| path == loaded_real))
                })
        })
        .ok_or_else(|| {
            XtaskError::failure(
                "desktop_host_artifact_missing",
                "Cargo did not report the host library loaded beside this launcher",
            )
        })?;
    launcher["hostLibrary"] =
        serde_json::json!({"path": loaded, "sha256": file_sha256(&loaded)?, "cargoArtifact": host});
    Ok(())
}

pub(super) fn verify_runtime_artifact(artifact: &serde_json::Value) -> Result<PathBuf> {
    let binary = artifact["executable"]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| {
            XtaskError::failure(
                "desktop_artifact_missing",
                "recorded launcher executable is missing",
            )
        })?;
    if artifact["executableSha256"].as_str() != Some(file_sha256(&binary)?.as_str()) {
        return Err(XtaskError::failure(
            "desktop_artifact_changed",
            "launcher changed since build evidence was recorded",
        ));
    }
    let expected = loaded_host_path(&binary)?;
    let recorded = artifact["hostLibrary"]["path"].as_str().map(PathBuf::from);
    if recorded.as_deref() != Some(expected.as_path()) {
        return Err(XtaskError::failure(
            "desktop_host_artifact_changed",
            "recorded host does not match the launcher's adjacent library",
        ));
    }
    let hash = file_sha256(&expected).map_err(|error| {
        XtaskError::failure(
            "desktop_host_artifact_missing",
            format!("cannot verify native host library: {error}"),
        )
    })?;
    if artifact["hostLibrary"]["sha256"].as_str() != Some(hash.as_str()) {
        return Err(XtaskError::failure(
            "desktop_host_artifact_changed",
            "native host library changed since build evidence was recorded",
        ));
    }
    Ok(binary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(command: &mut Command) {
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{command:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn actual_cargo_resolution_detects_cargo_home_patch_without_repo_config() {
        let temp = tempfile::tempdir().unwrap();
        let published = temp.path().join("published");
        fixture(&published, false);
        run(Command::new("git").current_dir(&published).arg("init"));
        run(Command::new("git")
            .current_dir(&published)
            .args(["add", "."]));
        run(Command::new("git").current_dir(&published).args([
            "-c",
            "user.name=Source Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "fixture",
        ]));
        let local = fixture(&temp.path().join("restored"), false);
        let consumer = temp.path().join("consumer");
        let cargo_home = temp.path().join("cargo-home");
        fs::create_dir_all(consumer.join("src")).unwrap();
        fs::create_dir_all(&cargo_home).unwrap();
        let git_url = url::Url::from_directory_path(&published)
            .unwrap()
            .to_string();
        fs::write(consumer.join("Cargo.toml"), format!(
            "[package]\nname = \"source-consumer\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\nnana-ui = {{ git = {git_url:?} }}\n"
        )).unwrap();
        fs::write(
            consumer.join("src/lib.rs"),
            "pub fn ready() { nana_ui::ready(); }\n",
        )
        .unwrap();
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let generate_lock = || {
            run(Command::new(&cargo)
                .current_dir(&consumer)
                .env("CARGO_HOME", &cargo_home)
                .arg("generate-lockfile"));
        };
        generate_lock();
        assert_eq!(
            resolve_local_nana_path(metadata_command(&consumer).env("CARGO_HOME", &cargo_home))
                .unwrap(),
            None
        );
        fs::write(
            cargo_home.join("config.toml"),
            format!(
                "[patch.{git_url:?}]\nnana-ui = {{ path = {:?} }}\n",
                local.to_str().unwrap()
            ),
        )
        .unwrap();
        generate_lock();
        let resolved =
            resolve_local_nana_path(metadata_command(&consumer).env("CARGO_HOME", &cargo_home))
                .unwrap()
                .unwrap();
        assert_eq!(resolved, fs::canonicalize(&local).unwrap());
        assert_eq!(
            fingerprint(&resolved).unwrap().workspace_path,
            fs::canonicalize(temp.path().join("restored")).unwrap()
        );
        assert!(!consumer.join(".cargo").exists());
    }

    #[test]
    fn source_selection_rejects_missing_ambiguous_or_incomplete_resolution() {
        let a = serde_json::json!({"name":"nana-ui", "id":"git+a", "source":"git+a"});
        let b = serde_json::json!({"name":"nana-ui", "id":"git+b", "source":"git+b"});
        let metadata = |packages: serde_json::Value, nodes: serde_json::Value| serde_json::json!({"packages":packages,"resolve":{"nodes":nodes}});
        assert_eq!(
            select_local_nana_path(&metadata(
                serde_json::json!([a, b]),
                serde_json::json!([{"id":"git+a"}])
            ))
            .unwrap(),
            None
        );
        assert_eq!(
            select_local_nana_path(&metadata(
                serde_json::json!([a, b]),
                serde_json::json!([{"id":"git+a"},{"id":"git+b"}])
            ))
            .unwrap_err()
            .code,
            "source_dependency_ambiguous"
        );
        assert_eq!(
            select_local_nana_path(&metadata(serde_json::json!([a]), serde_json::json!([])))
                .unwrap_err()
                .code,
            "source_dependency_missing"
        );
        assert_eq!(
            select_local_nana_path(&serde_json::json!({"packages":[a],"resolve":null}))
                .unwrap_err()
                .code,
            "source_metadata_invalid"
        );
    }

    fn fixture(root: &Path, reverse: bool) -> PathBuf {
        fs::create_dir_all(root.join("crates/nana-ui/src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/nana-ui\"]\n",
        )
        .unwrap();
        fs::write(root.join("Cargo.lock"), "version = 3\n").unwrap();
        let mut files = vec![
            (
                "Cargo.toml",
                "[package]\nname = \"nana-ui\"\nversion = \"0.1.0\"\n",
            ),
            ("src/lib.rs", "pub fn ready() {}\n"),
            ("src/shader.wgsl", "// fixture shader\n"),
        ];
        if reverse {
            files.reverse();
        }
        let package = root.join("crates/nana-ui");
        for (name, bytes) in files {
            fs::write(package.join(name), bytes).unwrap();
        }
        package
    }

    #[test]
    fn local_source_fingerprint_is_stable_and_ignores_build_outputs() {
        let temp = tempfile::tempdir().unwrap();
        let a = fixture(&temp.path().join("a"), false);
        let b = fixture(&temp.path().join("b"), true);
        let initial = fingerprint(&a).unwrap();
        assert_eq!(
            initial.source_sha256,
            fingerprint(&b).unwrap().source_sha256
        );
        assert_eq!(initial.source_file_count, 5);
        for directory in [
            a.join("target/debug"),
            temp.path().join("a/target/debug"),
            a.join(".git"),
            a.join("fixtures/vue-sfc-compat/node_modules/generated-package"),
            temp.path().join("a/unrelated-user-data"),
        ] {
            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join("ignored"), "not a source input").unwrap();
        }
        assert_eq!(initial, fingerprint(&a).unwrap());
        verify_unchanged(&Some(initial.clone()), &Some(fingerprint(&a).unwrap())).unwrap();
        fs::write(a.join("src/lib.rs"), "pub fn changed() {}\n").unwrap();
        let changed = fingerprint(&a).unwrap();
        assert_ne!(initial.source_sha256, changed.source_sha256);
        assert_eq!(
            verify_unchanged(&Some(initial), &Some(changed))
                .unwrap_err()
                .code,
            "source_changed_during_build"
        );
    }

    #[cfg(unix)]
    #[test]
    fn source_fingerprint_ignores_generated_dependency_links_but_rejects_source_links() {
        let temp = tempfile::tempdir().unwrap();
        let package = fixture(&temp.path().join("workspace"), false);
        let initial = fingerprint(&package).unwrap();
        let dependencies = package.join("fixtures/vue-sfc-compat/node_modules/@nanaui");
        fs::create_dir_all(&dependencies).unwrap();
        std::os::unix::fs::symlink(&package, dependencies.join("nanavue-components")).unwrap();
        fs::write(dependencies.join("generated.js"), "first install").unwrap();
        assert_eq!(fingerprint(&package).unwrap(), initial);
        fs::write(dependencies.join("generated.js"), "different install").unwrap();
        assert_eq!(fingerprint(&package).unwrap(), initial);

        let source_link = package.join("src/linked.rs");
        std::os::unix::fs::symlink(package.join("src/lib.rs"), &source_link).unwrap();
        assert_eq!(
            fingerprint(&package).unwrap_err().code,
            "source_fingerprint_failed"
        );
        fs::remove_file(source_link).unwrap();
        std::os::unix::fs::symlink(package.join("src"), package.join("linked-source")).unwrap();
        assert_eq!(
            fingerprint(&package).unwrap_err().code,
            "source_fingerprint_failed"
        );
    }

    #[test]
    fn local_source_fingerprint_detects_resource_rename_and_patch_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let a = fixture(&temp.path().join("a"), false);
        let b = fixture(&temp.path().join("b"), false);
        let before = fingerprint(&a).unwrap();
        assert!(verify_unchanged(&Some(before.clone()), &Some(fingerprint(&b).unwrap())).is_err());
        fs::rename(a.join("src/shader.wgsl"), a.join("src/renamed.wgsl")).unwrap();
        assert_ne!(before.source_sha256, fingerprint(&a).unwrap().source_sha256);
        assert!(verify_unchanged(&Some(before), &None).is_err());
    }

    #[test]
    fn file_sha256_records_exact_artifact_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let artifact = temp.path().join("artifact");
        fs::write(&artifact, b"abc").unwrap();
        assert_eq!(
            file_sha256(&artifact).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    fn runtime_fixture(root: &Path) -> (serde_json::Value, String, PathBuf) {
        fs::create_dir_all(root).unwrap();
        let binary = root.join(format!("liliacode{}", std::env::consts::EXE_SUFFIX));
        fs::write(&binary, b"launcher-v1").unwrap();
        let host = loaded_host_path(&binary).unwrap();
        fs::write(&host, b"host-v1").unwrap();
        let launcher = serde_json::json!({"reason":"compiler-artifact", "package_id":"desktop-fixture",
            "target":{"name":"liliacode"}, "executable":binary, "executableSha256":file_sha256(&binary).unwrap()});
        let host_artifact = serde_json::json!({"reason":"compiler-artifact", "package_id":"desktop-fixture",
            "target":{"name":"liliacode_host", "crate_types":["cdylib","rlib"]}, "filenames":[host]});
        (launcher, host_artifact.to_string(), host)
    }

    #[test]
    fn runtime_artifact_rejects_changed_or_missing_host_with_unchanged_launcher() {
        let temp = tempfile::tempdir().unwrap();
        let (mut artifact, output, host) = runtime_fixture(temp.path());
        record_host_artifact(&mut artifact, &output).unwrap();
        verify_runtime_artifact(&artifact).unwrap();
        let launcher_hash = artifact["executableSha256"].clone();
        fs::write(&host, b"host-v2").unwrap();
        assert_eq!(
            verify_runtime_artifact(&artifact).unwrap_err().code,
            "desktop_host_artifact_changed"
        );
        assert_eq!(
            launcher_hash.as_str(),
            Some(
                file_sha256(Path::new(artifact["executable"].as_str().unwrap()))
                    .unwrap()
                    .as_str()
            )
        );
        fs::remove_file(&host).unwrap();
        assert_eq!(
            verify_runtime_artifact(&artifact).unwrap_err().code,
            "desktop_host_artifact_missing"
        );
    }

    #[test]
    fn runtime_artifact_requires_cargo_reported_adjacent_cdylib_from_same_package() {
        let temp = tempfile::tempdir().unwrap();
        let (launcher, output, host) = runtime_fixture(&temp.path().join("runtime"));
        let reported: serde_json::Value = serde_json::from_str(&output).unwrap();
        for invalid in [
            serde_json::json!({"target":{"name":"liliacode_host", "crate_types":["rlib"]}, "package_id":"desktop-fixture", "reason":"compiler-artifact", "filenames":[host]}),
            serde_json::json!({"target":{"name":"liliacode_host", "crate_types":["cdylib"]}, "package_id":"other-package", "reason":"compiler-artifact", "filenames":[host]}),
        ] {
            assert!(record_host_artifact(&mut launcher.clone(), &invalid.to_string()).is_err());
        }
        let elsewhere = temp.path().join(host.file_name().unwrap());
        fs::copy(&host, &elsewhere).unwrap();
        let mut wrong_location = reported;
        wrong_location["filenames"] = serde_json::json!([elsewhere]);
        assert!(record_host_artifact(&mut launcher.clone(), &wrong_location.to_string()).is_err());
        let mut valid = launcher.clone();
        record_host_artifact(&mut valid, &output).unwrap();
        verify_runtime_artifact(&valid).unwrap();
        assert!(
            verify_runtime_artifact(&launcher).is_err(),
            "old launcher-only evidence must fail closed"
        );
    }

    #[test]
    fn runtime_artifact_rejects_changed_launcher_even_if_host_is_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let (mut artifact, output, _) = runtime_fixture(temp.path());
        record_host_artifact(&mut artifact, &output).unwrap();
        fs::write(artifact["executable"].as_str().unwrap(), b"launcher-v2").unwrap();
        assert_eq!(
            verify_runtime_artifact(&artifact).unwrap_err().code,
            "desktop_artifact_changed"
        );
    }
}
