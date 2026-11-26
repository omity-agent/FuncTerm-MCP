use alloc::sync::Arc;
use anyhow::{Context as _, Result, ensure};
use portable_pty::{Child, ChildKiller, ExitStatus};
use std::io;
use std::os::windows::io::{
    AsHandle as _, AsRawHandle as _, FromRawHandle as _, OwnedHandle, RawHandle,
};
use windows::Win32::{
    AssignProcessToJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JobObjectBasicAccountingInformation, QueryInformationJobObject,
};
use windows_spawn::Job;
pub(super) const SHELL_JOB_ENV: &str = "FUNCTERM_PTY_SHELL_JOB";
pub(in crate::engine) fn create_job() -> Result<Job> {
    let job = Job::create().context("failed to create shell containment Job")?;
    job.set_kill_on_close(true)
        .context("failed to enable shell Job cleanup")?;
    Ok(job)
}
pub(in crate::engine) fn attach(job: &Job, child: &dyn Child) -> Result<()> {
    let process = child
        .as_raw_handle()
        .context("shell child has no Windows process handle")?;
    unsafe { AssignProcessToJobObject(job.as_handle().as_raw_handle(), process) }
        .ok()
        .context("failed to attach shell process to its containment Job")?;
    tracing::debug!(process_id = ?child.process_id(), "shell containment Job attached");
    Ok(())
}
pub(super) fn inherited_shell_job() -> Result<Job> {
    let value = std::env::var(SHELL_JOB_ENV).context("ordinary PTY host has no shell Job")?;
    let raw: RawHandle =
        core::ptr::with_exposed_provenance_mut(value.parse::<isize>()?.cast_unsigned());
    ensure!(
        !raw.is_null(),
        "ordinary PTY host received a null shell Job handle"
    );
    let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    unsafe {
        QueryInformationJobObject(
            Some(raw),
            JobObjectBasicAccountingInformation,
            core::ptr::from_mut(&mut accounting).cast(),
            u32::try_from(size_of_val(&accounting))?,
            None,
        )
    }
    .ok()
    .context("invalid inherited shell Job handle")?;
    Job::from_handle(unsafe { OwnedHandle::from_raw_handle(raw) })
        .context("failed to adopt inherited shell Job")
}
#[derive(Debug)]
pub(super) struct HostedChild {
    child: windows_spawn::Child,
    job: Arc<Job>,
}
impl HostedChild {
    pub(super) const fn new(child: windows_spawn::Child, job: Arc<Job>) -> Self {
        Self { child, job }
    }
}
impl Child for HostedChild {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Ok(self.child.try_wait()?.map(ExitStatus::from))
    }
    fn wait(&mut self) -> io::Result<ExitStatus> {
        Ok(self.child.wait()?.into())
    }
    fn process_id(&self) -> Option<u32> {
        Some(self.child.id())
    }
    fn as_raw_handle(&self) -> Option<RawHandle> {
        Some(self.child.as_handle().as_raw_handle())
    }
}
impl ChildKiller for HostedChild {
    fn kill(&mut self) -> io::Result<()> {
        self.job.terminate(1)
    }
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(TreeKiller(Arc::clone(&self.job)))
    }
}
pub(super) fn job_killer(job: Job) -> Box<dyn ChildKiller + Send + Sync> {
    Box::new(TreeKiller(Arc::new(job)))
}
#[derive(Debug)]
struct TreeKiller(Arc<Job>);
impl ChildKiller for TreeKiller {
    fn kill(&mut self) -> io::Result<()> {
        self.0.terminate(1)
    }
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(Self(Arc::clone(&self.0)))
    }
}
