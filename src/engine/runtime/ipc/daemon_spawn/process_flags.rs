use super::StartupProcess;
use anyhow::{Context as _, Result};
use std::process::Command;
#[cfg(windows)]
pub(super) fn spawn_detached(
    mut command: Command,
    _process_kind: StartupProcess,
) -> Result<std::process::Child> {
    use std::os::windows::process::CommandExt as _;
    let job = current_job_state();
    command
        .creation_flags(windows_creation_flags(job))
        .inherit_handles(false);
    command.spawn().context("CreateProcessW failed")
}
#[cfg(windows)]
pub(super) fn needs_shell_parent_daemon_spawn() -> bool {
    matches!(current_job_state(), JobState::ForbidsBreakaway)
}
#[cfg(not(windows))]
pub(super) const fn needs_shell_parent_daemon_spawn() -> bool {
    false
}
#[cfg(windows)]
const fn windows_creation_flags(job: JobState) -> u32 {
    windows_creation_flags_for_job(job)
}
#[cfg(windows)]
const fn windows_creation_flags_for_job(job: JobState) -> u32 {
    use windows::Win32::{CREATE_BREAKAWAY_FROM_JOB, DETACHED_PROCESS};
    let base = DETACHED_PROCESS.cast_unsigned();
    match job {
        JobState::AllowsBreakaway => base | CREATE_BREAKAWAY_FROM_JOB.cast_unsigned(),
        JobState::NotInJob | JobState::ForbidsBreakaway | JobState::Unknown => base,
    }
}
#[cfg(unix)]
pub(super) fn spawn_detached(
    mut command: Command,
    _process_kind: StartupProcess,
) -> Result<std::process::Child> {
    use std::os::unix::process::CommandExt as _;
    command.setsid(true);
    command.spawn().context("failed to spawn detached process")
}
#[cfg(not(any(unix, windows)))]
pub(super) fn spawn_detached(
    mut command: Command,
    _process_kind: StartupProcess,
) -> Result<std::process::Child> {
    command.spawn().context("failed to spawn detached process")
}
#[cfg(windows)]
#[derive(Clone, Copy)]
enum JobState {
    NotInJob,
    AllowsBreakaway,
    ForbidsBreakaway,
    Unknown,
}
#[cfg(windows)]
fn current_job_state() -> JobState {
    use windows::Win32::{
        GetCurrentProcess, IsProcessInJob, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
        JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, QueryInformationJobObject,
    };
    use windows::core::BOOL;
    let mut in_job = BOOL::default();
    let process = unsafe { GetCurrentProcess() };
    if unsafe { IsProcessInJob(process, None, &raw mut in_job) }
        .ok()
        .is_err()
    {
        return JobState::Unknown;
    }
    if !in_job.as_bool() {
        return JobState::NotInJob;
    }
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    let Ok(size) = u32::try_from(core::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
    else {
        return JobState::Unknown;
    };
    if unsafe {
        QueryInformationJobObject(
            None,
            JobObjectExtendedLimitInformation,
            core::ptr::from_mut(&mut limits).cast(),
            size,
            None,
        )
    }
    .ok()
    .is_err()
    {
        return JobState::Unknown;
    }
    let limit_flags = limits.BasicLimitInformation.LimitFlags;
    if limit_flags
        & (JOB_OBJECT_LIMIT_BREAKAWAY_OK | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK).cast_unsigned()
        == 0
    {
        JobState::ForbidsBreakaway
    } else {
        JobState::AllowsBreakaway
    }
}
#[cfg(windows)]
pub(super) fn spawn_with_shell_parent(mut command: Command) -> Result<std::process::Child> {
    use std::os::windows::process::CommandExt as _;
    use windows::Win32::PROCESS_CREATE_PROCESS;
    let shell_process =
        crate::engine::privileges::desktop_host::open(u32::try_from(PROCESS_CREATE_PROCESS)?)?;
    command
        .creation_flags(windows_creation_flags_for_job(JobState::NotInJob))
        .inherit_handles(false);
    crate::engine::privileges::desktop_host::spawn(&mut command, &shell_process)
}
#[cfg(not(windows))]
pub(super) fn spawn_with_shell_parent(_command: Command) -> Result<std::process::Child> {
    anyhow::bail!("Windows shell parent launch is only available on Windows")
}
