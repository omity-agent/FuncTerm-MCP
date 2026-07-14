use super::StartupEvent;
use anyhow::{Context as _, Result, bail};
use portable_pty::Child;
use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle, RawHandle};
use std::sync::mpsc;
use std::thread;
use windows_sys::Win32::Foundation::{
    DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE, WAIT_FAILED, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, INFINITE, WaitForSingleObject};
pub(super) fn monitor_child(
    child: &(dyn Child + Send + Sync),
    sender: mpsc::Sender<StartupEvent>,
) -> Result<()> {
    let raw_handle = child
        .as_raw_handle()
        .context("shell child has no Windows process handle")?;
    let duplicate = duplicate_handle(raw_handle)?;
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
fn duplicate_handle(raw_handle: RawHandle) -> Result<OwnedHandle> {
    let current_process = unsafe { GetCurrentProcess() };
    let mut duplicate: HANDLE = core::ptr::null_mut();
    let succeeded = unsafe {
        DuplicateHandle(
            current_process,
            raw_handle,
            current_process,
            &raw mut duplicate,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    };
    if succeeded == 0_i32 {
        return Err(std::io::Error::last_os_error()).context("failed to duplicate shell handle");
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(duplicate) })
}
fn wait_for_process(handle: OwnedHandle) -> Result<()> {
    let wait_result = unsafe { WaitForSingleObject(handle.as_raw_handle(), INFINITE) };
    drop(handle);
    match wait_result {
        WAIT_OBJECT_0 => Ok(()),
        WAIT_FAILED => {
            Err(std::io::Error::last_os_error()).context("failed to wait for shell process")
        }
        unexpected => bail!("unexpected shell process wait result {unexpected}"),
    }
}
#[cfg(test)]
mod tests {
    use super::{StartupEvent, monitor_child};
    use core::time::Duration;
    use portable_pty::Child;
    use std::sync::mpsc;
    #[test]
    fn process_monitor_reports_exit_without_polling() {
        let process = std::process::Command::new("whoami.exe")
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut child: Box<dyn Child + Send + Sync> = Box::new(process);
        let (sender, receiver) = mpsc::channel();
        monitor_child(child.as_ref(), sender).unwrap();
        let event = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        let StartupEvent::ProcessExited(result) = event else {
            panic!("expected process exit event");
        };
        result.unwrap();
        child.wait().unwrap();
    }
}
