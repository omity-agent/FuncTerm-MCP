use super::startup::StartupEvent;
use crate::runtime::session::terminal::Terminal;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use portable_pty::Child;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle as _, BorrowedHandle, OwnedHandle};
use std::sync::mpsc;
use std::thread;
#[cfg(windows)]
use windows::Win32::{INFINITE, WAIT_FAILED, WAIT_OBJECT_0, WaitForSingleObject};
pub(super) fn monitor_child(
    child: &(dyn Child + Send + Sync),
    sender: mpsc::Sender<StartupEvent>,
    screen: Arc<Terminal>,
) -> Result<()> {
    let process = observe_process(child)?;
    let spawn_result = thread::Builder::new()
        .name("functerm-shell-lifetime".to_owned())
        .spawn(move || {
            let result = wait_for_process(process);
            match result.as_ref() {
                Ok(&()) => screen.process_exited(),
                Err(error) => {
                    screen.reader_failed(&format!("shell process monitor failed: {error:#}"));
                }
            }
            let _sent = sender.send(StartupEvent::ProcessExited(result));
        });
    spawn_result.context("failed to start shell process monitor")?;
    Ok(())
}
#[cfg(windows)]
fn observe_process(child: &(dyn Child + Send + Sync)) -> Result<OwnedHandle> {
    let raw_handle = child
        .as_raw_handle()
        .context("shell child has no Windows process handle")?;
    let borrowed = unsafe { BorrowedHandle::borrow_raw(raw_handle) };
    borrowed
        .try_clone_to_owned()
        .context("failed to duplicate shell handle")
}
#[cfg(windows)]
fn wait_for_process(handle: OwnedHandle) -> Result<()> {
    let windows_handle = handle.as_raw_handle();
    let wait_result = unsafe { WaitForSingleObject(windows_handle, INFINITE) };
    let wait_error = (wait_result == WAIT_FAILED).then(std::io::Error::last_os_error);
    drop(handle);
    if let Some(error) = wait_error {
        return Err(error).context("failed to wait for shell process");
    }
    match wait_result {
        result if result == WAIT_OBJECT_0.0.cast_unsigned() => Ok(()),
        unexpected => bail!("unexpected shell process wait result {unexpected:?}"),
    }
}
#[cfg(unix)]
fn observe_process(child: &(dyn Child + Send + Sync)) -> Result<rustix::process::Pid> {
    let raw = child
        .process_id()
        .context("shell child has no Unix process id")?;
    let pid = i32::try_from(raw).context("shell process id exceeds the Unix pid range")?;
    rustix::process::Pid::from_raw(pid).context("shell process id is zero")
}
#[cfg(unix)]
fn wait_for_process(pid: rustix::process::Pid) -> Result<()> {
    use rustix::io::{Errno, retry_on_intr};
    use rustix::process::{WaitId, WaitIdOptions, waitid};
    let result = retry_on_intr(|| {
        waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
        )
    });
    match result {
        Ok(Some(_)) | Err(Errno::CHILD) => Ok(()),
        Ok(None) => bail!("blocking shell process observation returned no status"),
        Err(error) => Err(error).context("failed to observe shell process exit"),
    }
}
