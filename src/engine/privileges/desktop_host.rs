use anyhow::{Context as _, Result, ensure};
use std::os::windows::io::{FromRawHandle as _, OwnedHandle};
use windows::Win32::{GetShellWindow, GetWindowThreadProcessId, OpenProcess};
pub(crate) fn open(access: u32) -> Result<OwnedHandle> {
    let shell_window = unsafe { GetShellWindow() };
    ensure!(
        !shell_window.is_null(),
        "Windows shell window was not found"
    );
    let mut process_id = 0_u32;
    let thread_id = unsafe { GetWindowThreadProcessId(shell_window, Some(&raw mut process_id)) };
    if thread_id == 0 {
        return Err(windows::core::Error::from_thread())
            .context("failed to resolve Windows shell process");
    }
    ensure!(process_id != 0, "Windows shell process id was not found");
    let process = unsafe { OpenProcess(access, false, process_id) };
    if process.is_null() {
        return Err(windows::core::Error::from_thread())
            .context("failed to open Windows shell process");
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(process) })
}
