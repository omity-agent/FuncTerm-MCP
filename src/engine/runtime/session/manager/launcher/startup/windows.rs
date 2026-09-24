use super::StartupEvent;
use anyhow::{Context as _, Result, bail};
use portable_pty::Child;
use std::os::windows::io::{AsRawHandle as _, BorrowedHandle, OwnedHandle};
use std::sync::mpsc;
use std::thread;
use windows::Win32::Foundation::{HANDLE, WAIT_FAILED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{INFINITE, WaitForSingleObject};
pub(super) fn monitor_child(
    child: &(dyn Child + Send + Sync),
    sender: mpsc::Sender<StartupEvent>,
) -> Result<()> {
    let raw_handle = child
        .as_raw_handle()
        .context("shell child has no Windows process handle")?;
    let borrowed = unsafe { BorrowedHandle::borrow_raw(raw_handle) };
    let duplicate = borrowed
        .try_clone_to_owned()
        .context("failed to duplicate shell handle")?;
    let spawn_result = thread::Builder::new()
        .name("functerm-shell-startup".to_owned())
        .spawn(move || {
            let result = wait_for_process(duplicate);
            let _sent = sender.send(StartupEvent::ProcessExited(result));
        });
    if let Err(error) = spawn_result {
        return Err(error).context("failed to start shell process monitor");
    }
    Ok(())
}
fn wait_for_process(handle: OwnedHandle) -> Result<()> {
    let windows_handle = HANDLE(handle.as_raw_handle());
    let wait_result = unsafe { WaitForSingleObject(windows_handle, INFINITE) };
    drop(handle);
    match wait_result {
        WAIT_OBJECT_0 => Ok(()),
        WAIT_FAILED => {
            Err(std::io::Error::last_os_error()).context("failed to wait for shell process")
        }
        unexpected => bail!("unexpected shell process wait result {unexpected:?}"),
    }
}
#[cfg(test)]
#[path = "../../../../../../../tests/unit/platform/conpty_reply.rs"]
mod tests;
