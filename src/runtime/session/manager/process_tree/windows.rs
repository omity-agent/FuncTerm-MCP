use anyhow::{Context as _, Error, Result};
use core::ffi::c_void;
use portable_pty::Child;
use windows_sys::Win32::Foundation::{BOOL, CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
const FALSE: BOOL = 0;
const NULL_HANDLE: HANDLE = 0;
pub(crate) struct ProcessTree {
    job: HANDLE,
}
unsafe impl Send for ProcessTree {}
unsafe impl Sync for ProcessTree {}
impl ProcessTree {
    pub(crate) fn new() -> Result<Self> {
        let job = unsafe { CreateJobObjectW(core::ptr::null(), core::ptr::null()) };
        if job == NULL_HANDLE {
            return Err(last_error("failed to create shell cleanup job"));
        }
        let tree = Self { job };
        if let Err(error) = tree.enable_kill_on_close() {
            drop(tree);
            return Err(error);
        }
        Ok(tree)
    }
    #[expect(
        clippy::as_conversions,
        reason = "windows-sys 0.48 models HANDLE as isize while RawHandle is a pointer"
    )]
    pub(crate) fn attach(&self, child: &dyn Child) -> Result<()> {
        let process = child
            .as_raw_handle()
            .context("shell child does not expose a process handle")?;
        let assigned = unsafe { AssignProcessToJobObject(self.job, process as HANDLE) };
        if assigned == FALSE {
            return Err(last_error("failed to assign shell child to cleanup job"));
        }
        Ok(())
    }
    pub(crate) fn terminate(&self) -> Result<()> {
        let terminated = unsafe { TerminateJobObject(self.job, 1_u32) };
        if terminated == FALSE {
            return Err(last_error("failed to terminate shell cleanup job"));
        }
        Ok(())
    }
    fn enable_kill_on_close(&self) -> Result<()> {
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { core::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let byte_len = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
            .context("job limit information size exceeds u32")?;
        let configured = unsafe {
            SetInformationJobObject(
                self.job,
                JobObjectExtendedLimitInformation,
                core::ptr::from_ref(&limits).cast::<c_void>(),
                byte_len,
            )
        };
        if configured == FALSE {
            return Err(last_error("failed to enable cleanup job kill-on-close"));
        }
        Ok(())
    }
}
#[expect(
    clippy::missing_trait_methods,
    reason = "Drop only needs the regular destructor for this type"
)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        let closed = unsafe { CloseHandle(self.job) };
        if closed == FALSE {
            eprintln!(
                "failed to close shell cleanup job: {}",
                std::io::Error::last_os_error()
            );
        }
    }
}
fn last_error(message: &str) -> Error {
    let error = std::io::Error::last_os_error();
    anyhow::anyhow!("{message}: {error}")
}
#[cfg(test)]
#[expect(
    clippy::inline_modules,
    reason = "Rust skill permits inline modules guarded by cfg(test)"
)]
mod tests {
    use super::ProcessTree;
    use std::process::Command;
    #[test]
    fn closing_job_terminates_attached_process() {
        let mut child = Command::new("cmd.exe")
            .args(["/C", "ping -n 30 127.0.0.1 > nul"])
            .spawn()
            .unwrap();
        {
            let tree = ProcessTree::new().unwrap();
            tree.attach(&child).unwrap();
        }
        let status = child.wait().unwrap();
        assert!(!status.success());
    }
}
