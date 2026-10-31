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
    let mut in_job = 0_i32;
    let process = unsafe { GetCurrentProcess() };
    if unsafe { IsProcessInJob(process, None, &raw mut in_job) }
        .ok()
        .is_err()
    {
        return JobState::Unknown;
    }
    if in_job == 0_i32 {
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
pub(super) fn spawn_with_shell_parent(service_name: &str) -> Result<()> {
    use windows::Win32::{
        PROCESS_CREATE_PROCESS, PROCESS_DUP_HANDLE, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_spawn::{CreationFlags, ParentProcess, SpawnOptions, Stdio};
    let access = u32::try_from(
        PROCESS_CREATE_PROCESS | PROCESS_DUP_HANDLE | PROCESS_QUERY_LIMITED_INFORMATION,
    )?;
    let shell_process = crate::engine::privileges::desktop_host::open(access)?;
    let parent = ParentProcess::from_handle(shell_process)?;
    let executable = std::env::current_exe().context("failed to locate current executable")?;
    let mut command = windows_spawn::Command::new(executable);
    command
        .arg(super::DAEMON_SUBCOMMAND)
        .env("FUNCTERM_DAEMON_SERVICE_NAME", service_name)
        .env_remove(super::READY_STDOUT_ENV)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let options = SpawnOptions::new()
        .parent_process(&parent)
        .creation_flags(CreationFlags::DETACHED_PROCESS);
    let _daemon = command
        .spawn_with(options)
        .context("failed to spawn detached daemon through Windows shell parent")?;
    Ok(())
}
