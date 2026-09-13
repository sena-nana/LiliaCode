use std::io;

#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, BorrowedHandle, OwnedHandle};

pub(crate) struct ProcessKiller {
    #[cfg(windows)]
    handle: OwnedHandle,
    #[cfg(not(windows))]
    inner: Box<dyn portable_pty::ChildKiller + Send + Sync>,
}

impl ProcessKiller {
    pub(crate) fn new(child: &mut dyn portable_pty::Child) -> io::Result<Self> {
        #[cfg(windows)]
        {
            let raw = child.as_raw_handle().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Unsupported,
                    "PTY process has no Windows handle",
                )
            })?;
            // The child owns this handle until its wait thread starts.
            let borrowed = unsafe { BorrowedHandle::borrow_raw(raw) };
            match borrowed.try_clone_to_owned() {
                Ok(handle) => Ok(Self { handle }),
                Err(error) => {
                    let _ = terminate_handle(raw);
                    let _ = child.wait();
                    Err(error)
                }
            }
        }
        #[cfg(not(windows))]
        {
            Ok(Self {
                inner: child.clone_killer(),
            })
        }
    }

    pub(crate) fn kill(&mut self) -> io::Result<()> {
        #[cfg(windows)]
        {
            terminate_handle(self.handle.as_raw_handle())
        }
        #[cfg(not(windows))]
        {
            self.inner.kill()
        }
    }
}

#[cfg(windows)]
fn terminate_handle(raw: std::os::windows::io::RawHandle) -> io::Result<()> {
    use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{TerminateProcess, WaitForSingleObject};

    let handle = HANDLE(raw);
    // An owned process handle stays bound to this process even after its PID is reused.
    match unsafe { TerminateProcess(handle, 1) } {
        Ok(()) => Ok(()),
        Err(error) => {
            if unsafe { WaitForSingleObject(handle, 0) } == WAIT_OBJECT_0 {
                Ok(())
            } else {
                Err(io::Error::other(error))
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    #[test]
    fn termination_reports_success_and_wait_observes_the_exit() {
        let mut child = Command::new("cmd.exe")
            .args(["/D", "/Q", "/K"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut killer = ProcessKiller::new(&mut child).unwrap();
        assert!(child.try_wait().unwrap().is_none());
        killer.kill().unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(1));
    }

    #[test]
    fn natural_exit_and_released_child_handle_do_not_turn_cancellation_into_an_error() {
        let mut child = Command::new("cmd.exe")
            .args(["/D", "/Q", "/C", "exit /b 7"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut killer = ProcessKiller::new(&mut child).unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(7));
        drop(child);
        killer.kill().unwrap();
    }
}
