#[cfg(all(windows, not(test)))]
pub(crate) fn set_system_awake(active: bool) -> Result<(), String> {
    use windows::Win32::System::Power::{
        SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED,
    };

    let flags = if active {
        ES_CONTINUOUS | ES_SYSTEM_REQUIRED
    } else {
        ES_CONTINUOUS
    };
    let previous = unsafe { SetThreadExecutionState(flags) };
    if previous.0 == 0 {
        Err("SetThreadExecutionState failed".to_string())
    } else {
        Ok(())
    }
}

#[cfg(any(not(windows), test))]
pub(crate) fn set_system_awake(_active: bool) -> Result<(), String> {
    Ok(())
}
