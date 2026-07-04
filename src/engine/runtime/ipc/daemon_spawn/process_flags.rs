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
    command.creation_flags(windows_creation_flags(job));
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
    use windows_sys::Win32::System::Threading::{
        CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS,
    };
    let base = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
    match job {
        JobState::AllowsBreakaway => base | CREATE_BREAKAWAY_FROM_JOB,
        JobState::NotInJob | JobState::ForbidsBreakaway | JobState::Unknown => base,
    }
}
#[cfg(unix)]
pub(super) fn spawn_detached(
    mut command: Command,
    _process_kind: StartupProcess,
) -> Result<std::process::Child> {
    use std::os::unix::process::CommandExt as _;
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
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
    use windows_sys::Win32::Foundation::TRUE;
    use windows_sys::Win32::System::JobObjects::{
        IsProcessInJob, JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        QueryInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut in_job = 0_i32;
    let process = unsafe { GetCurrentProcess() };
    if unsafe { IsProcessInJob(process, core::ptr::null_mut(), &raw mut in_job) } != TRUE {
        return JobState::Unknown;
    }
    if in_job != TRUE {
        return JobState::NotInJob;
    }
    let mut limits = unsafe { core::mem::zeroed::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() };
    let Ok(size) = u32::try_from(core::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
    else {
        return JobState::Unknown;
    };
    if unsafe {
        QueryInformationJobObject(
            core::ptr::null_mut(),
            JobObjectExtendedLimitInformation,
            core::ptr::from_mut(&mut limits).cast(),
            size,
            core::ptr::null_mut(),
        )
    } != TRUE
    {
        return JobState::Unknown;
    }
    let limit_flags = limits.BasicLimitInformation.LimitFlags;
    if limit_flags & (JOB_OBJECT_LIMIT_BREAKAWAY_OK | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK) == 0 {
        JobState::ForbidsBreakaway
    } else {
        JobState::AllowsBreakaway
    }
}
#[cfg(windows)]
pub(super) fn spawn_with_shell_parent(mut command: Command) -> Result<std::process::Child> {
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
    use std::os::windows::process::{CommandExt as _, ProcThreadAttributeList};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROC_THREAD_ATTRIBUTE_PARENT_PROCESS, PROCESS_CREATE_PROCESS,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetShellWindow, GetWindowThreadProcessId};
    let shell_window = unsafe { GetShellWindow() };
    anyhow::ensure!(
        !shell_window.is_null(),
        "Windows shell window was not found"
    );
    let mut shell_process_id = 0_u32;
    unsafe {
        GetWindowThreadProcessId(shell_window, &raw mut shell_process_id);
    }
    anyhow::ensure!(
        shell_process_id != 0,
        "Windows shell process id was not found"
    );
    let process = unsafe { OpenProcess(PROCESS_CREATE_PROCESS, 0, shell_process_id) };
    anyhow::ensure!(!process.is_null(), "failed to open Windows shell process");
    let shell_process = unsafe { OwnedHandle::from_raw_handle(process) };
    let parent = shell_process.as_raw_handle();
    let parent_attribute = usize::try_from(PROC_THREAD_ATTRIBUTE_PARENT_PROCESS)
        .context("parent process attribute does not fit usize")?;
    let attributes = unsafe {
        ProcThreadAttributeList::build().raw_attribute(
            parent_attribute,
            core::ptr::addr_of!(parent),
            core::mem::size_of_val(&parent),
        )
    }
    .finish()
    .context("failed to build launcher process attributes")?;
    command.creation_flags(windows_creation_flags_for_job(JobState::NotInJob));
    command
        .spawn_with_attributes(&attributes)
        .context("failed to spawn process through Windows shell process")
}
#[cfg(not(windows))]
pub(super) fn spawn_with_shell_parent(_command: Command) -> Result<std::process::Child> {
    anyhow::bail!("Windows shell parent launch is only available on Windows")
}
#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn windows_detached_flags_break_away_from_parent_job() {
        use windows_sys::Win32::System::Threading::CREATE_BREAKAWAY_FROM_JOB;
        let flags = super::windows_creation_flags_for_job(super::JobState::AllowsBreakaway);
        assert_ne!(flags & CREATE_BREAKAWAY_FROM_JOB, 0);
    }
    #[cfg(windows)]
    #[test]
    fn windows_detached_flags_avoid_forbidden_breakaway() {
        use windows_sys::Win32::System::Threading::CREATE_BREAKAWAY_FROM_JOB;
        let flags = super::windows_creation_flags_for_job(super::JobState::ForbidsBreakaway);
        assert_eq!(flags & CREATE_BREAKAWAY_FROM_JOB, 0);
    }
    #[cfg(windows)]
    #[test]
    fn windows_detached_flags_do_not_break_away_outside_jobs() {
        use windows_sys::Win32::System::Threading::CREATE_BREAKAWAY_FROM_JOB;
        let flags = super::windows_creation_flags_for_job(super::JobState::NotInJob);
        assert_eq!(flags & CREATE_BREAKAWAY_FROM_JOB, 0);
    }
}
