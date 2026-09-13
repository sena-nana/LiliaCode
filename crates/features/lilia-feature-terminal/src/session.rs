use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

use lilia_contracts::{ProjectId, TaskId};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::process::ProcessKiller;
use crate::TerminalEvents;

const TERMINAL_SCROLLBACK_ROWS: usize = 10_000;
const MIN_TERMINAL_ROWS: u16 = 2;
const MAX_TERMINAL_ROWS: u16 = 500;
const MIN_TERMINAL_COLUMNS: u16 = 8;
const MAX_TERMINAL_COLUMNS: u16 = 1_000;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DesktopTerminalSessionId(String);

impl DesktopTerminalSessionId {
    fn new() -> Self {
        Self(format!("terminal-{}", Uuid::new_v4()))
    }

    pub fn from_stored(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DesktopTerminalSessionId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum DesktopTerminalScope {
    Project(ProjectId),
    Task(TaskId),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTerminalCommand {
    pub program: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub label: Option<String>,
}

impl DesktopTerminalCommand {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTerminalLaunch {
    pub scope: DesktopTerminalScope,
    pub command: Option<DesktopTerminalCommand>,
    pub rows: u16,
    pub columns: u16,
}

impl DesktopTerminalLaunch {
    pub fn shell(scope: DesktopTerminalScope) -> Self {
        Self {
            scope,
            command: None,
            rows: 24,
            columns: 80,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DesktopTerminalProcessState {
    Running,
    Terminating,
    Exited {
        success: bool,
        exit_code: u32,
        signal: Option<String>,
    },
    Failed {
        message: String,
    },
    Restored,
}

impl DesktopTerminalProcessState {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running | Self::Terminating)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum DesktopTerminalColor {
    Default,
    Indexed(u8),
    Rgb([u8; 3]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTerminalStyle {
    pub foreground: DesktopTerminalColor,
    pub background: DesktopTerminalColor,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTerminalRow {
    pub text: String,
    pub styles: Vec<DesktopTerminalStyleSpan>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTerminalStyleSpan {
    pub start: usize,
    pub end: usize,
    pub style: DesktopTerminalStyle,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTerminalSnapshot {
    pub id: DesktopTerminalSessionId,
    pub scope: DesktopTerminalScope,
    pub cwd: PathBuf,
    pub command_label: String,
    pub process_id: Option<u32>,
    pub rows: u16,
    pub columns: u16,
    pub cursor_row: u16,
    pub cursor_column: u16,
    pub cursor_visible: bool,
    pub scrollback_position: usize,
    pub maximum_scrollback_position: usize,
    pub screen: Vec<DesktopTerminalRow>,
    #[serde(default)]
    pub cells: Vec<lilia_contracts::TerminalGridCell>,
    #[serde(default)]
    pub application_cursor: bool,
    #[serde(default)]
    pub bracketed_paste: bool,
    pub process: DesktopTerminalProcessState,
    pub revision: u64,
    pub output_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTerminalRestoration {
    pub id: DesktopTerminalSessionId,
    pub scope: DesktopTerminalScope,
    pub cwd: PathBuf,
    pub command_label: String,
    pub rows: u16,
    pub columns: u16,
}

impl DesktopTerminalRestoration {
    pub fn from_snapshot(snapshot: &DesktopTerminalSnapshot) -> Self {
        Self {
            id: snapshot.id.clone(),
            scope: snapshot.scope.clone(),
            cwd: snapshot.cwd.clone(),
            command_label: snapshot.command_label.clone(),
            rows: snapshot.rows,
            columns: snapshot.columns,
        }
    }

    pub fn snapshot(&self) -> DesktopTerminalSnapshot {
        DesktopTerminalSnapshot {
            id: self.id.clone(),
            scope: self.scope.clone(),
            cwd: self.cwd.clone(),
            command_label: self.command_label.clone(),
            process_id: None,
            rows: self.rows,
            columns: self.columns,
            cursor_row: 0,
            cursor_column: 0,
            cursor_visible: false,
            scrollback_position: 0,
            maximum_scrollback_position: 0,
            screen: vec![DesktopTerminalRow {
                text: "该终端会话已随上次应用退出而结束。".to_owned(),
                styles: Vec::new(),
            }],
            process: DesktopTerminalProcessState::Restored,
            cells: Vec::new(),
            application_cursor: false,
            bracketed_paste: false,
            revision: 0,
            output_error: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DesktopTerminalError {
    #[error("terminal rows must be between {MIN_TERMINAL_ROWS} and {MAX_TERMINAL_ROWS}")]
    InvalidRows,
    #[error("terminal columns must be between {MIN_TERMINAL_COLUMNS} and {MAX_TERMINAL_COLUMNS}")]
    InvalidColumns,
    #[error("terminal command program must not be empty or contain control characters")]
    InvalidProgram,
    #[error("terminal command label must not be empty or contain control characters")]
    InvalidCommandLabel,
    #[error("terminal environment key `{0}` is invalid")]
    InvalidEnvironmentKey(String),
    #[error("terminal scope has no workspace directory")]
    MissingWorkspace,
    #[error("terminal workspace `{0}` is not a directory")]
    WorkspaceNotDirectory(PathBuf),
    #[error("terminal session `{0}` does not exist")]
    SessionNotFound(DesktopTerminalSessionId),
    #[error("terminal session `{0}` is no longer running")]
    SessionNotRunning(DesktopTerminalSessionId),
    #[error("terminal session `{0}` is still running")]
    SessionStillRunning(DesktopTerminalSessionId),
    #[error("terminal {operation} failed: {message}")]
    Operation {
        operation: &'static str,
        message: String,
    },
    #[error("terminal state is unavailable")]
    StateUnavailable,
}

pub struct DesktopTerminalService {
    sessions: Mutex<HashMap<DesktopTerminalSessionId, DesktopTerminalSession>>,
}

impl Default for DesktopTerminalService {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

impl DesktopTerminalService {
    /// Starts a PTY in `cwd` and registers it.
    pub fn launch(
        &self,
        launch: DesktopTerminalLaunch,
        cwd: PathBuf,
        events: Arc<dyn TerminalEvents>,
    ) -> Result<DesktopTerminalSnapshot, DesktopTerminalError> {
        validate_size(launch.rows, launch.columns)?;
        validate_command(launch.command.as_ref())?;
        let session = DesktopTerminalSession::spawn(launch, cwd, events)?;
        let session_id = session.id.clone();
        let registration = session.registration.clone();
        self.sessions
            .lock()
            .map_err(|_| DesktopTerminalError::StateUnavailable)?
            .insert(session.id.clone(), session);
        mark_terminal_registered(&registration);
        self.snapshot(&session_id, 0)
    }

    pub fn snapshot(
        &self,
        session_id: &DesktopTerminalSessionId,
        scrollback_position: usize,
    ) -> Result<DesktopTerminalSnapshot, DesktopTerminalError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| DesktopTerminalError::StateUnavailable)?;
        sessions
            .get(session_id)
            .ok_or_else(|| DesktopTerminalError::SessionNotFound(session_id.clone()))?
            .snapshot(scrollback_position)
    }

    pub fn list(&self) -> Result<Vec<DesktopTerminalSnapshot>, DesktopTerminalError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| DesktopTerminalError::StateUnavailable)?;
        let mut snapshots = sessions
            .values()
            .map(|session| session.snapshot(0))
            .collect::<Result<Vec<_>, _>>()?;
        snapshots.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(snapshots)
    }

    pub fn write(
        &self,
        session_id: &DesktopTerminalSessionId,
        input: &[u8],
    ) -> Result<(), DesktopTerminalError> {
        if input.is_empty() {
            return Ok(());
        }
        let writer = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| DesktopTerminalError::StateUnavailable)?;
            let session = sessions
                .get(session_id)
                .ok_or_else(|| DesktopTerminalError::SessionNotFound(session_id.clone()))?;
            session.ensure_running()?;
            session.writer.clone()
        };
        let mut writer = writer
            .lock()
            .map_err(|_| DesktopTerminalError::StateUnavailable)?;
        writer
            .write_all(input)
            .and_then(|()| writer.flush())
            .map_err(|error| operation_error("write input", error))
    }

    pub fn resize(
        &self,
        session_id: &DesktopTerminalSessionId,
        rows: u16,
        columns: u16,
    ) -> Result<DesktopTerminalSnapshot, DesktopTerminalError> {
        validate_size(rows, columns)?;
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| DesktopTerminalError::StateUnavailable)?;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| DesktopTerminalError::SessionNotFound(session_id.clone()))?;
        session
            .master
            .resize(pty_size(rows, columns))
            .map_err(|error| DesktopTerminalError::Operation {
                operation: "resize",
                message: error.to_string(),
            })?;
        let mut state = session
            .state
            .lock()
            .map_err(|_| DesktopTerminalError::StateUnavailable)?;
        state.parser.screen_mut().set_size(rows, columns);
        state.revision = state.revision.saturating_add(1);
        drop(state);
        session.snapshot(0)
    }

    pub fn terminate(
        &self,
        session_id: &DesktopTerminalSessionId,
    ) -> Result<(), DesktopTerminalError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| DesktopTerminalError::StateUnavailable)?;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| DesktopTerminalError::SessionNotFound(session_id.clone()))?;
        session.ensure_running()?;
        session
            .killer
            .kill()
            .map_err(|error| operation_error("terminate", error))?;
        let mut state = session
            .state
            .lock()
            .map_err(|_| DesktopTerminalError::StateUnavailable)?;
        if state.process.is_running() {
            state.process = DesktopTerminalProcessState::Terminating;
            state.revision = state.revision.saturating_add(1);
        }
        Ok(())
    }

    /// Drops a finished session. A running session must be terminated first so
    /// no process is left without an owner.
    pub fn forget(
        &self,
        session_id: &DesktopTerminalSessionId,
    ) -> Result<(), DesktopTerminalError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| DesktopTerminalError::StateUnavailable)?;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| DesktopTerminalError::SessionNotFound(session_id.clone()))?;
        if session.is_running()? {
            return Err(DesktopTerminalError::SessionStillRunning(
                session_id.clone(),
            ));
        }
        let removed = sessions.remove(session_id);
        drop(sessions);
        drop(removed);
        Ok(())
    }
}

struct DesktopTerminalSession {
    id: DesktopTerminalSessionId,
    scope: DesktopTerminalScope,
    cwd: PathBuf,
    command_label: String,
    process_id: Option<u32>,
    master: Box<dyn MasterPty + Send>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    killer: ProcessKiller,
    state: Arc<Mutex<DesktopTerminalRuntimeState>>,
    registration: TerminalRegistration,
}

type TerminalRegistration = Arc<(Mutex<bool>, Condvar)>;

struct DesktopTerminalRuntimeState {
    parser: vt100::Parser<TerminalResponses>,
    process: DesktopTerminalProcessState,
    revision: u64,
    output_error: Option<String>,
}

#[derive(Default)]
struct TerminalResponses {
    bytes: Vec<u8>,
}

impl vt100::Callbacks for TerminalResponses {
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        first: Option<u8>,
        second: Option<u8>,
        params: &[&[u16]],
        command: char,
    ) {
        if command != 'n' || second.is_some() || params.len() != 1 {
            return;
        }
        match (first, params[0]) {
            (None, [5]) => self.bytes.extend_from_slice(b"\x1b[0n"),
            (None | Some(b'?'), [6]) => {
                let (row, column) = screen.cursor_position();
                let prefix = if first.is_some() { "?" } else { "" };
                self.bytes.extend_from_slice(
                    format!("\x1b[{prefix}{};{}R", row + 1, column + 1).as_bytes(),
                );
            }
            _ => {}
        }
    }
}

impl DesktopTerminalSession {
    fn spawn(
        launch: DesktopTerminalLaunch,
        cwd: PathBuf,
        events: Arc<dyn TerminalEvents>,
    ) -> Result<Self, DesktopTerminalError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(pty_size(launch.rows, launch.columns))
            .map_err(|error| DesktopTerminalError::Operation {
                operation: "open PTY",
                message: error.to_string(),
            })?;
        let mut reader =
            pair.master
                .try_clone_reader()
                .map_err(|error| DesktopTerminalError::Operation {
                    operation: "open output reader",
                    message: error.to_string(),
                })?;
        let writer =
            pair.master
                .take_writer()
                .map_err(|error| DesktopTerminalError::Operation {
                    operation: "open input writer",
                    message: error.to_string(),
                })?;
        let writer = Arc::new(Mutex::new(writer));
        let (command, command_label) = command_builder(&launch, &cwd);
        let mut child =
            pair.slave
                .spawn_command(command)
                .map_err(|error| DesktopTerminalError::Operation {
                    operation: "spawn process",
                    message: error.to_string(),
                })?;
        drop(pair.slave);

        let process_id = child.process_id();
        let mut killer = ProcessKiller::new(child.as_mut())
            .map_err(|error| operation_error("own process handle", error))?;
        let state = Arc::new(Mutex::new(DesktopTerminalRuntimeState {
            parser: vt100::Parser::new_with_callbacks(
                launch.rows,
                launch.columns,
                TERMINAL_SCROLLBACK_ROWS,
                TerminalResponses::default(),
            ),
            process: DesktopTerminalProcessState::Running,
            revision: 1,
            output_error: None,
        }));
        let registration = Arc::new((Mutex::new(false), Condvar::new()));

        let session_id = DesktopTerminalSessionId::new();
        let wait_state = state.clone();
        let wait_events = events.clone();
        let wait_session_id = session_id.clone();
        let wait_registration = registration.clone();
        let wait_spawn = std::thread::Builder::new()
            .name(format!(
                "lilia-terminal-wait-{}",
                process_id.unwrap_or_default()
            ))
            .spawn(move || {
                let process = match child.wait() {
                    Ok(status) => DesktopTerminalProcessState::Exited {
                        success: status.success(),
                        exit_code: status.exit_code(),
                        signal: status.signal().map(str::to_owned),
                    },
                    Err(error) => DesktopTerminalProcessState::Failed {
                        message: error.to_string(),
                    },
                };
                let revision = wait_state.lock().ok().map(|mut state| {
                    state.process = process;
                    state.revision = state.revision.saturating_add(1);
                    state.revision
                });
                if let Some(revision) = revision {
                    wait_until_terminal_registered(&wait_registration);
                    wait_events.changed(&wait_session_id, revision);
                }
            });
        if let Err(error) = wait_spawn {
            mark_terminal_registered(&registration);
            let _ = killer.kill();
            return Err(DesktopTerminalError::Operation {
                operation: "start process waiter",
                message: error.to_string(),
            });
        }

        let reader_state = state.clone();
        let response_writer = writer.clone();
        let reader_events = events;
        let reader_session_id = session_id.clone();
        let reader_registration = registration.clone();
        let reader_spawn = std::thread::Builder::new()
            .name(format!(
                "lilia-terminal-read-{}",
                process_id.unwrap_or_default()
            ))
            .spawn(move || {
                let mut buffer = [0_u8; 16 * 1024];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(read) => {
                            let (mut revision, response) = {
                                let Ok(mut state) = reader_state.lock() else {
                                    break;
                                };
                                state.parser.process(&buffer[..read]);
                                state.revision = state.revision.saturating_add(1);
                                let response =
                                    std::mem::take(&mut state.parser.callbacks_mut().bytes);
                                (state.revision, response)
                            };
                            if !response.is_empty() {
                                let written = match response_writer.lock() {
                                    Ok(mut writer) => {
                                        writer.write_all(&response).and_then(|_| writer.flush())
                                    }
                                    Err(_) => Err(std::io::Error::other(
                                        "terminal input writer is unavailable",
                                    )),
                                };
                                if let Err(error) = written {
                                    if let Ok(mut state) = reader_state.lock() {
                                        state.output_error = Some(error.to_string());
                                        state.revision = state.revision.saturating_add(1);
                                        revision = state.revision;
                                    }
                                }
                            }
                            wait_until_terminal_registered(&reader_registration);
                            reader_events.changed(&reader_session_id, revision);
                        }
                        Err(error) => {
                            let revision = reader_state.lock().ok().and_then(|mut state| {
                                if !state.process.is_running() {
                                    return None;
                                }
                                state.output_error = Some(error.to_string());
                                state.revision = state.revision.saturating_add(1);
                                Some(state.revision)
                            });
                            if let Some(revision) = revision {
                                wait_until_terminal_registered(&reader_registration);
                                reader_events.changed(&reader_session_id, revision);
                            }
                            break;
                        }
                    }
                }
            });
        if let Err(error) = reader_spawn {
            mark_terminal_registered(&registration);
            let _ = killer.kill();
            return Err(DesktopTerminalError::Operation {
                operation: "start output reader",
                message: error.to_string(),
            });
        }

        Ok(Self {
            id: session_id,
            scope: launch.scope,
            cwd,
            command_label,
            process_id,
            master: pair.master,
            writer,
            killer,
            state,
            registration,
        })
    }

    fn snapshot(
        &self,
        scrollback_position: usize,
    ) -> Result<DesktopTerminalSnapshot, DesktopTerminalError> {
        let state = self
            .state
            .lock()
            .map_err(|_| DesktopTerminalError::StateUnavailable)?;
        let mut screen = state.parser.screen().clone();
        let mut maximum_screen = screen.clone();
        maximum_screen.set_scrollback(usize::MAX);
        let maximum_scrollback_position = maximum_screen.scrollback();
        screen.set_scrollback(scrollback_position);
        let (rows, columns) = screen.size();
        let (cursor_row, cursor_column) = screen.cursor_position();
        Ok(DesktopTerminalSnapshot {
            id: self.id.clone(),
            scope: self.scope.clone(),
            cwd: self.cwd.clone(),
            command_label: self.command_label.clone(),
            process_id: self.process_id,
            rows,
            columns,
            cursor_row,
            cursor_column,
            cursor_visible: !screen.hide_cursor(),
            scrollback_position: screen.scrollback(),
            maximum_scrollback_position,
            screen: snapshot_rows(&screen),
            cells: snapshot_cells(&screen),
            application_cursor: screen.application_cursor(),
            bracketed_paste: screen.bracketed_paste(),
            process: state.process.clone(),
            revision: state.revision,
            output_error: state.output_error.clone(),
        })
    }

    fn is_running(&self) -> Result<bool, DesktopTerminalError> {
        self.state
            .lock()
            .map(|state| state.process.is_running())
            .map_err(|_| DesktopTerminalError::StateUnavailable)
    }

    fn ensure_running(&self) -> Result<(), DesktopTerminalError> {
        self.is_running()?
            .then_some(())
            .ok_or_else(|| DesktopTerminalError::SessionNotRunning(self.id.clone()))
    }
}

fn mark_terminal_registered(registration: &TerminalRegistration) {
    let (registered, signal) = &**registration;
    if let Ok(mut registered) = registered.lock() {
        *registered = true;
        signal.notify_all();
    }
}

fn wait_until_terminal_registered(registration: &TerminalRegistration) {
    let (registered, signal) = &**registration;
    let Ok(mut registered) = registered.lock() else {
        return;
    };
    while !*registered {
        let Ok(next) = signal.wait(registered) else {
            return;
        };
        registered = next;
    }
}

impl Drop for DesktopTerminalService {
    fn drop(&mut self) {
        if let Ok(sessions) = self.sessions.get_mut() {
            for session in sessions.values_mut() {
                if session.is_running().unwrap_or(false) {
                    let _ = session.killer.kill();
                }
            }
        }
    }
}

fn validate_size(rows: u16, columns: u16) -> Result<(), DesktopTerminalError> {
    if !(MIN_TERMINAL_ROWS..=MAX_TERMINAL_ROWS).contains(&rows) {
        return Err(DesktopTerminalError::InvalidRows);
    }
    if !(MIN_TERMINAL_COLUMNS..=MAX_TERMINAL_COLUMNS).contains(&columns) {
        return Err(DesktopTerminalError::InvalidColumns);
    }
    Ok(())
}

fn validate_command(command: Option<&DesktopTerminalCommand>) -> Result<(), DesktopTerminalError> {
    let Some(command) = command else {
        return Ok(());
    };
    if command.program.trim().is_empty() || command.program.chars().any(char::is_control) {
        return Err(DesktopTerminalError::InvalidProgram);
    }
    if command
        .label
        .as_deref()
        .is_some_and(|label| label.trim().is_empty() || label.chars().any(char::is_control))
    {
        return Err(DesktopTerminalError::InvalidCommandLabel);
    }
    if let Some(key) = command
        .environment
        .keys()
        .find(|key| key.is_empty() || key.contains('=') || key.chars().any(char::is_control))
    {
        return Err(DesktopTerminalError::InvalidEnvironmentKey(key.clone()));
    }
    Ok(())
}

pub fn canonical_directory(path: &Path) -> Result<PathBuf, DesktopTerminalError> {
    let canonical =
        std::fs::canonicalize(path).map_err(|error| DesktopTerminalError::Operation {
            operation: "resolve workspace",
            message: error.to_string(),
        })?;
    if !canonical.is_dir() {
        return Err(DesktopTerminalError::WorkspaceNotDirectory(canonical));
    }
    Ok(canonical)
}

fn command_builder(launch: &DesktopTerminalLaunch, cwd: &Path) -> (CommandBuilder, String) {
    let (mut command, label) = if let Some(specification) = &launch.command {
        let mut command = CommandBuilder::new(&specification.program);
        command.args(&specification.arguments);
        for (key, value) in &specification.environment {
            command.env(key, value);
        }
        (
            command,
            specification
                .label
                .clone()
                .unwrap_or_else(|| specification.program.clone()),
        )
    } else {
        (CommandBuilder::new_default_prog(), "Shell".to_owned())
    };
    command.cwd(cwd);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    (command, label)
}

fn pty_size(rows: u16, columns: u16) -> PtySize {
    PtySize {
        rows,
        cols: columns,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn snapshot_rows(screen: &vt100::Screen) -> Vec<DesktopTerminalRow> {
    let (rows, columns) = screen.size();
    (0..rows)
        .map(|row| {
            let mut text = String::new();
            let mut styles = Vec::new();
            let mut current_style = None;
            let mut current_start = 0;
            for column in 0..columns {
                let Some(cell) = screen.cell(row, column) else {
                    continue;
                };
                if cell.is_wide_continuation() {
                    continue;
                }
                let style = terminal_style(cell);
                let start = text.len();
                if cell.has_contents() {
                    text.push_str(cell.contents());
                } else {
                    text.push(' ');
                }
                if current_style != Some(style) {
                    if let Some(previous) = current_style {
                        styles.push(DesktopTerminalStyleSpan {
                            start: current_start,
                            end: start,
                            style: previous,
                        });
                    }
                    current_style = Some(style);
                    current_start = start;
                }
            }
            if let Some(style) = current_style {
                styles.push(DesktopTerminalStyleSpan {
                    start: current_start,
                    end: text.len(),
                    style,
                });
            }
            DesktopTerminalRow { text, styles }
        })
        .collect()
}

fn snapshot_cells(screen: &vt100::Screen) -> Vec<lilia_contracts::TerminalGridCell> {
    use lilia_contracts::{TerminalCellColor, TerminalGridCell};
    let color = |value| match value {
        vt100::Color::Default => TerminalCellColor::Default,
        vt100::Color::Idx(index) => TerminalCellColor::Indexed(index),
        vt100::Color::Rgb(r, g, b) => TerminalCellColor::Rgb([r, g, b]),
    };
    let (rows, columns) = screen.size();
    (0..rows)
        .flat_map(|row| {
            (0..columns).map(move |column| {
                let Some(cell) = screen.cell(row, column) else {
                    return TerminalGridCell {
                        text: " ".into(),
                        width: 1,
                        ..Default::default()
                    };
                };
                TerminalGridCell {
                    text: cell.contents().to_owned(),
                    width: if cell.is_wide_continuation() {
                        0
                    } else if cell.is_wide() {
                        2
                    } else {
                        1
                    },
                    foreground: color(cell.fgcolor()),
                    background: color(cell.bgcolor()),
                    bold: cell.bold(),
                    dim: cell.dim(),
                    italic: cell.italic(),
                    underline: cell.underline(),
                    inverse: cell.inverse(),
                }
            })
        })
        .collect()
}

fn terminal_style(cell: &vt100::Cell) -> DesktopTerminalStyle {
    DesktopTerminalStyle {
        foreground: terminal_color(cell.fgcolor()),
        background: terminal_color(cell.bgcolor()),
        bold: cell.bold(),
        dim: cell.dim(),
        italic: cell.italic(),
        underline: cell.underline(),
        inverse: cell.inverse(),
    }
}

#[cfg(test)]
mod grid_tests {
    use super::*;

    #[test]
    fn terminal_queries_survive_read_boundaries_and_use_the_live_cursor() {
        let mut parser = vt100::Parser::new_with_callbacks(8, 80, 0, TerminalResponses::default());
        parser.process(b"normal output [6n");
        assert!(parser.callbacks().bytes.is_empty());
        parser.process(b"\x1b[3;9H\x1b[");
        parser.process(b"6");
        assert!(parser.callbacks().bytes.is_empty());
        parser.process(b"n");
        assert_eq!(
            std::mem::take(&mut parser.callbacks_mut().bytes),
            b"\x1b[3;9R"
        );
        parser.process(b"X\x1b[?6n\x1b[5n");
        assert_eq!(
            std::mem::take(&mut parser.callbacks_mut().bytes),
            b"\x1b[?3;10R\x1b[0n"
        );
        parser.process(b"\x1b]0;literal [6n\x07");
        assert!(parser.callbacks().bytes.is_empty());
    }

    #[test]
    fn real_pty_subscriber_can_synchronously_read_committed_output_and_exit() {
        use std::sync::{mpsc, Weak};
        use std::time::{Duration, Instant};
        struct SnapshotSubscriber {
            service: Weak<DesktopTerminalService>,
            changed: mpsc::Sender<(u64, Result<DesktopTerminalSnapshot, DesktopTerminalError>)>,
        }
        impl TerminalEvents for SnapshotSubscriber {
            fn changed(&self, id: &DesktopTerminalSessionId, revision: u64) {
                let Some(service) = self.service.upgrade() else {
                    return;
                };
                let snapshot = service.snapshot(id, 0);
                let _ = self.changed.send((revision, snapshot));
            }
        }
        let service = Arc::new(DesktopTerminalService::default());
        let (changes, observed) = mpsc::channel();
        let events = Arc::new(SnapshotSubscriber {
            service: Arc::downgrade(&service),
            changed: changes,
        });
        let scope = DesktopTerminalScope::Task(TaskId::new("reentrant-terminal-test").unwrap());
        #[cfg(windows)]
        let command = DesktopTerminalCommand {
            arguments: vec![
                "/D".into(),
                "/C".into(),
                "echo committed-pty-output & exit /b 7".into(),
            ],
            ..DesktopTerminalCommand::new("cmd.exe")
        };
        #[cfg(not(windows))]
        let command = DesktopTerminalCommand {
            arguments: vec![
                "-c".into(),
                "printf 'committed-pty-output\n'; exit 7".into(),
            ],
            ..DesktopTerminalCommand::new("/bin/sh")
        };
        let launch = DesktopTerminalLaunch {
            scope: scope.clone(),
            command: Some(command),
            rows: 8,
            columns: 80,
        };
        let worker_service = service.clone();
        let (launched, launch_result) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = launched.send(worker_service.launch(launch, std::env::temp_dir(), events));
        });
        let timeout = Duration::from_secs(15);
        let started = launch_result
            .recv_timeout(timeout)
            .expect("terminal launch or its snapshot deadlocked")
            .expect("real PTY launch succeeds");
        let deadline = Instant::now() + timeout;
        let mut saw_output = false;
        let mut saw_exit = false;
        let mut last_observed = None;
        while !(saw_output && saw_exit) {
            let (revision, snapshot) = observed
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|error| panic!("subscriber did not observe output and exit: {error:?}; output={saw_output}, exit={saw_exit}, last={last_observed:?}"));
            let snapshot = snapshot.expect("published session is registered and readable");
            assert_eq!(snapshot.id, started.id);
            assert_eq!(snapshot.scope, scope);
            assert!(revision > 1 && snapshot.revision >= revision);
            saw_output |= snapshot
                .screen
                .iter()
                .any(|row| row.text.contains("committed-pty-output"));
            last_observed = Some((
                snapshot.revision,
                snapshot.process.clone(),
                snapshot
                    .screen
                    .iter()
                    .map(|row| row.text.clone())
                    .collect::<Vec<_>>(),
            ));
            saw_exit |= matches!(
                snapshot.process,
                DesktopTerminalProcessState::Exited {
                    success: false,
                    exit_code: 7,
                    ..
                }
            );
        }
        let final_snapshot = service.snapshot(&started.id, 0).unwrap();
        assert!(!final_snapshot.process.is_running());
        service.forget(&started.id).unwrap();
    }

    #[test]
    fn grid_preserves_wide_continuations_combining_text_and_ansi_style() {
        let mut parser = vt100::Parser::new(2, 8, 0);
        parser.process("\u{1b}[31;1m中e\u{301}\u{1b}[0m!".as_bytes());
        let cells = snapshot_cells(parser.screen());
        assert_eq!(cells.len(), 16);
        assert_eq!((&*cells[0].text, cells[0].width), ("中", 2));
        assert_eq!(cells[1].width, 0);
        assert_eq!((&*cells[2].text, cells[2].width), ("e\u{301}", 1));
        assert_eq!(
            cells[0].foreground,
            lilia_contracts::TerminalCellColor::Indexed(1)
        );
        assert!(cells[2].bold);
        assert!(!cells[3].bold);
        assert_eq!(cells[3].text, "!");
    }
}

fn terminal_color(color: vt100::Color) -> DesktopTerminalColor {
    match color {
        vt100::Color::Default => DesktopTerminalColor::Default,
        vt100::Color::Idx(index) => DesktopTerminalColor::Indexed(index),
        vt100::Color::Rgb(red, green, blue) => DesktopTerminalColor::Rgb([red, green, blue]),
    }
}

fn operation_error(operation: &'static str, error: std::io::Error) -> DesktopTerminalError {
    DesktopTerminalError::Operation {
        operation,
        message: error.to_string(),
    }
}
