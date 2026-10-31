use anyhow::{Context as _, Result};
use portable_pty::{Child, ChildKiller};
use std::os::windows::io::{AsHandle as _, AsRawHandle as _};
use windows::Win32::AssignProcessToJobObject;
use windows_spawn::Job;
pub(super) enum ProcessGuard {
    Hosted(Box<dyn ChildKiller + Send + Sync>),
    Assigned(Job),
}
impl ProcessGuard {
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "borrow owning Job handles or killers in place without moving them"
    )]
    pub(super) fn terminate(&mut self) -> Result<()> {
        match self {
            Self::Hosted(killer) => killer
                .kill()
                .context("failed to terminate hosted shell Job"),
            Self::Assigned(job) => job
                .terminate(1)
                .context("failed to terminate assigned shell Job"),
        }
    }
}
pub(super) fn assign(
    child: &dyn Child,
    existing: Option<Box<dyn ChildKiller + Send + Sync>>,
) -> Result<ProcessGuard> {
    if let Some(killer) = existing {
        return Ok(ProcessGuard::Hosted(killer));
    }
    let process = child
        .as_raw_handle()
        .context("shell child has no Windows process handle")?;
    let job = Job::create().context("failed to create shell containment Job")?;
    job.set_kill_on_close(true)
        .context("failed to enable shell Job cleanup")?;
    unsafe { AssignProcessToJobObject(job.as_handle().as_raw_handle(), process) }
        .ok()
        .context("failed to attach shell process to its containment Job")?;
    tracing::debug!(process_id = ?child.process_id(), "shell containment Job attached");
    Ok(ProcessGuard::Assigned(job))
}
