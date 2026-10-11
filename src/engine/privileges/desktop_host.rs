use anyhow::{Context as _, Result, ensure};
use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
use std::os::windows::process::{CommandExt as _, ProcThreadAttributeList};
use std::process::{Child, Command};
use windows::Win32::System::Threading::{
    OpenProcess, PROC_THREAD_ATTRIBUTE_PARENT_PROCESS, PROCESS_ACCESS_RIGHTS,
};
use windows::Win32::UI::WindowsAndMessaging::{GetShellWindow, GetWindowThreadProcessId};
pub(crate) fn open(access: PROCESS_ACCESS_RIGHTS) -> Result<OwnedHandle> {
    let shell_window = unsafe { GetShellWindow() };
    ensure!(
        !shell_window.is_invalid(),
        "Windows shell window was not found"
    );
    let mut process_id = 0_u32;
    unsafe {
        GetWindowThreadProcessId(shell_window, Some(&raw mut process_id));
    }
    ensure!(process_id != 0, "Windows shell process id was not found");
    let process = unsafe { OpenProcess(access, false, process_id) }
        .context("failed to open Windows shell process")?;
    Ok(unsafe { OwnedHandle::from_raw_handle(process.0) })
}
pub(crate) fn spawn(command: &mut Command, parent: &OwnedHandle) -> Result<Child> {
    let handle = parent.as_raw_handle();
    let attribute = usize::try_from(PROC_THREAD_ATTRIBUTE_PARENT_PROCESS)?;
    let attributes = unsafe {
        ProcThreadAttributeList::build().raw_attribute(
            attribute,
            core::ptr::addr_of!(handle),
            core::mem::size_of_val(&handle),
        )
    }
    .finish()
    .context("failed to prepare Windows shell parent attributes")?;
    command.inherit_handles(false);
    command
        .spawn_with_attributes(&attributes)
        .context("failed to spawn process through Windows shell parent")
}
