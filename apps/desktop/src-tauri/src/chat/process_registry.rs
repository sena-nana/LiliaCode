use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout};
use std::sync::{Arc, Mutex};

use serde_json::Value as JsonValue;

#[derive(Debug)]
pub(crate) enum JsonlProcessPoll {
    Pending,
    StdoutLine(String),
    Exited(JsonlProcessExit),
}

#[derive(Debug)]
pub(crate) struct JsonlProcessExit {
    pub(crate) success: bool,
    pub(crate) stderr_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JsonlProcessStdinStatus {
    Ready { bytes: usize },
    Unavailable,
    WriteFailed { bytes: usize },
}

struct JsonlProcessSession {
    child: Child,
    stdout: Option<BufReader<ChildStdout>>,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    stderr: Option<std::process::ChildStderr>,
    finished: bool,
}

pub(crate) struct JsonlProcessRegistry {
    sessions: Mutex<HashMap<String, JsonlProcessSession>>,
    next_id: Mutex<u64>,
}

impl JsonlProcessRegistry {
    pub(crate) fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            next_id: Mutex::new(0),
        }
    }

    pub(crate) fn start(
        &self,
        mut child: Child,
        initial_payload: &JsonValue,
    ) -> std::io::Result<String> {
        let mut next_id = self.next_id.lock().unwrap();
        *next_id += 1;
        let session_id = format!("jsonl-process-{}", *next_id);
        let stdin = child.stdin.take().map(|stdin| Arc::new(Mutex::new(stdin)));
        let stdout = child.stdout.take().map(BufReader::new);
        let stderr = child.stderr.take();
        let session = JsonlProcessSession {
            child,
            stdout,
            stdin: stdin.clone(),
            stderr,
            finished: false,
        };
        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.clone(), session);
        if let Some(stdin) = stdin {
            let mut line = serde_json::to_string(initial_payload)?;
            line.push('\n');
            let mut stdin = stdin.lock().unwrap();
            stdin.write_all(line.as_bytes())?;
            stdin.flush()?;
        }
        Ok(session_id)
    }

    pub(crate) fn stdin_handle(&self, session_id: &str) -> Option<Arc<Mutex<ChildStdin>>> {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .and_then(|session| session.stdin.clone())
    }

    pub(crate) fn take_stdin_handle(&self, session_id: &str) -> Option<Arc<Mutex<ChildStdin>>> {
        self.sessions
            .lock()
            .unwrap()
            .get_mut(session_id)
            .and_then(|session| session.stdin.take())
    }

    pub(crate) fn release_child_handle(&self, session_id: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(session) = sessions.get_mut(session_id) else {
            return;
        };
        if session.finished {
            return;
        }
        if matches!(session.child.try_wait(), Ok(None)) {
            let _ = session.child.kill();
        }
        let _ = session.child.wait();
        session.finished = true;
    }

    pub(crate) fn stdin_status(&self, session_id: &str) -> Option<JsonlProcessStdinStatus> {
        let handle = self.stdin_handle(session_id)?;
        let status = match handle.lock() {
            Ok(mut stdin) => match stdin.flush() {
                Ok(()) => Some(JsonlProcessStdinStatus::Ready { bytes: 0 }),
                Err(_) => Some(JsonlProcessStdinStatus::WriteFailed { bytes: 0 }),
            },
            Err(_) => Some(JsonlProcessStdinStatus::Unavailable),
        };
        status
    }

    pub(crate) fn stdout_available(&self, session_id: &str) -> bool {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .and_then(|session| session.stdout.as_ref())
            .is_some()
    }

    pub(crate) fn poll(&self, session_id: &str) -> Option<JsonlProcessPoll> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.get_mut(session_id)?;
        if session.finished {
            return Some(JsonlProcessPoll::Pending);
        }
        if let Some(stdout) = session.stdout.as_mut() {
            let mut line = String::new();
            match stdout.read_line(&mut line) {
                Ok(0) => {}
                Ok(_) => return Some(JsonlProcessPoll::StdoutLine(line)),
                Err(_) => {}
            }
        }
        match session.child.try_wait() {
            Ok(Some(status)) => {
                session.finished = true;
                Some(JsonlProcessPoll::Exited(JsonlProcessExit {
                    success: status.success(),
                    stderr_text: read_stderr(session.stderr.take()),
                }))
            }
            Ok(None) => Some(JsonlProcessPoll::Pending),
            Err(_) => {
                session.finished = true;
                Some(JsonlProcessPoll::Exited(JsonlProcessExit {
                    success: false,
                    stderr_text: "failed to poll child process".to_string(),
                }))
            }
        }
    }

    pub(crate) fn terminate(&self, session_id: &str) -> Result<bool, String> {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(session) = sessions.get_mut(session_id) else {
            return Ok(false);
        };
        if session.finished {
            return Ok(false);
        }
        session.child.kill().map_err(|err| err.to_string())?;
        session.finished = true;
        Ok(true)
    }

    pub(crate) fn is_active(&self, session_id: &str) -> bool {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .is_some_and(|session| !session.finished)
    }

    pub(crate) fn remove(&self, session_id: &str) -> Option<()> {
        self.sessions.lock().unwrap().remove(session_id).map(|_| ())
    }
}

impl Default for JsonlProcessRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn read_stderr(stderr: Option<std::process::ChildStderr>) -> String {
    let Some(mut stderr) = stderr else {
        return String::new();
    };
    let mut text = String::new();
    let _ = stderr.read_to_string(&mut text);
    text
}
