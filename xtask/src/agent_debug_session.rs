use super::*;

impl Session {
    pub(super) fn restart(&mut self) -> Result {
        let artifact_path = self.run_dir.join("build-artifact.json");
        let artifact: Value =
            serde_json::from_slice(&fs::read(&artifact_path).map_err(|error| {
                XtaskError::io(
                    "restart_artifact_failed",
                    "read original build artifact",
                    error,
                )
            })?)
            .map_err(|error| XtaskError::failure("restart_artifact_failed", error.to_string()))?;
        let binary = source::verify_runtime_artifact(&artifact)?;
        let old_pid = self.pid();
        if self
            .child
            .try_wait()
            .map_err(|error| {
                XtaskError::io(
                    "restart_process_failed",
                    "inspect original desktop process",
                    error,
                )
            })?
            .is_none()
        {
            self.child.kill().map_err(|error| {
                XtaskError::io(
                    "restart_process_failed",
                    "stop isolated desktop process",
                    error,
                )
            })?;
        }
        let exit = self.child.wait().map_err(|error| {
            XtaskError::io(
                "restart_process_failed",
                "reap isolated desktop process",
                error,
            )
        })?;
        let ready = self.run_dir.join("ready.txt");
        match fs::remove_file(&ready) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(XtaskError::io(
                    "restart_ready_failed",
                    "remove obsolete ready address",
                    error,
                ))
            }
        }
        let stamp = timestamp();
        let source: Value =
            serde_json::from_slice(&fs::read(self.run_dir.join("source.json")).map_err(
                |error| XtaskError::io("restart_source_failed", "read requested viewport", error),
            )?)
            .map_err(|error| XtaskError::failure("restart_source_failed", error.to_string()))?;
        let viewport = &source["requestedViewport"];
        if let (Some(width), Some(height), Some(theme)) = (
            viewport["width"].as_u64(),
            viewport["height"].as_u64(),
            viewport["theme"].as_str(),
        ) {
            write_json(
                &self.run_dir.join("home/main-window-state.json"),
                &serde_json::json!({
                    "x": 40, "y": 40, "width": width, "height": height,
                    "scaleFactor": 1.0, "maximized": false
                }),
            )?;
            fs::write(self.run_dir.join("home/appearance.theme"), theme).map_err(|error| {
                XtaskError::io("restart_theme_failed", "restore matrix theme", error)
            })?;
        }
        let stdout = fs::File::create(self.run_dir.join(format!("restart-{stamp}.stdout.log")))
            .map_err(|error| {
                XtaskError::io("debug_log_failed", "create restart stdout log", error)
            })?;
        let stderr = fs::File::create(self.run_dir.join(format!("restart-{stamp}.stderr.log")))
            .map_err(|error| {
                XtaskError::io("debug_log_failed", "create restart stderr log", error)
            })?;
        source::verify_runtime_artifact(&artifact)?;
        let started = Instant::now();
        let mut child = Command::new(&binary)
            .current_dir(repo_root()?)
            .env("LILIA_HOME", self.run_dir.join("home"))
            .env("LILIA_AGENT_DEBUG", "1")
            .env("LILIA_AGENT_DEBUG_ADDR", "127.0.0.1:0")
            .env("LILIA_AGENT_DEBUG_READY", &ready)
            .env("LILIA_AGENT_DEBUG_SEED", "1")
            .env("LILIA_AGENT_DEBUG_RESUME", "1")
            .env("LILIA_AGENT_DEBUG_EPHEMERAL_CREDENTIALS", "1")
            .env(
                "LILIA_JOURNAL_PATH",
                self.run_dir.join(format!("restart-{stamp}.journal.jsonl")),
            )
            .env(
                "LILIA_AGENT_DEBUG_MODEL_ENDPOINT",
                self.model_fixture
                    .as_ref()
                    .map_or(DISCARD_MODEL_ENDPOINT, |fixture| fixture.endpoint()),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|error| {
                XtaskError::io(
                    "desktop_launch_failed",
                    "restart original desktop binary",
                    error,
                )
            })?;
        let address = match wait_ready(&ready) {
            Ok(address) => address,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        self.child = child;
        self.address = address;
        self.startup_ms = started.elapsed().as_secs_f64() * 1000.0;
        source::verify_runtime_artifact(&artifact)?;
        write_json(
            &self.run_dir.join(format!("restart-{stamp}.json")),
            &serde_json::json!({
                "oldPid": old_pid, "oldProcessReaped": true, "oldExit": exit.to_string(),
                "newPid": self.pid(), "home": self.run_dir.join("home"),
                "executableSha256": artifact["executableSha256"], "hostLibrary": artifact["hostLibrary"],
                "runtimeArtifactVerified": true, "businessFixturesReplayed": false,
                "startupMs": self.startup_ms,
                "requestedViewport": viewport,
            }),
        )
    }
}
