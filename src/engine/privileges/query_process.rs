use super::ShellIdentity;
use crate::runtime::protocol::EnvironmentSnapshot;
use anyhow::{Context as _, Result, ensure};
use core::time::Duration;
use std::path::Path;
use std::process::ExitStatus;
const TIMEOUT: Duration = Duration::from_secs(5);
const OUTPUT_LIMIT: u64 = 4096;
#[cfg(windows)]
pub(crate) fn query(
    program: &Path,
    arguments: &[&str],
    cwd: &Path,
    environment: &EnvironmentSnapshot,
    identity: &ShellIdentity,
) -> Result<Vec<u8>> {
    use std::io::Read as _;
    use std::os::windows::io::{AsHandle as _, AsRawHandle as _};
    use windows::Win32::{WAIT_FAILED, WAIT_OBJECT_0, WaitForSingleObject};
    use windows_spawn::{Command, CreationFlags, DropPolicy, ParentProcess, SpawnOptions, Stdio};
    let helper = std::env::current_exe().context("failed to resolve shell probe helper")?;
    let mut command = Command::new(helper);
    command.arg("internal-query-shell");
    if identity.needs_normal_host() {
        command.arg("--require-ordinary");
    }
    command
        .arg("--")
        .arg(program)
        .args(arguments)
        .current_dir(cwd)
        .env_clear()
        .envs(
            environment
                .variables()
                .into_iter()
                .filter(|entry| !entry.0.as_encoded_bytes().starts_with(b"=")),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut options = SpawnOptions::new()
        .creation_flags(CreationFlags::NO_WINDOW)
        .drop_policy(DropPolicy::KillTree);
    let parent;
    if identity.needs_normal_host() {
        parent = ParentProcess::from_handle(super::access_token::normal_parent()?)?;
        options = options.parent_process(&parent);
    }
    let mut child = command
        .spawn_with(options)
        .context("failed to launch shell probe helper")?;
    let wait = unsafe {
        WaitForSingleObject(
            child.as_handle().as_raw_handle(),
            u32::try_from(TIMEOUT.as_millis())?,
        )
    };
    if wait == WAIT_FAILED {
        return Err(std::io::Error::last_os_error()).context("failed to wait for shell probe");
    }
    ensure!(
        wait == WAIT_OBJECT_0.0.cast_unsigned(),
        "shell probe did not exit within {TIMEOUT:?}"
    );
    let status = child.wait()?;
    let stdout = child
        .stdout
        .take()
        .context("shell probe has no output pipe")?;
    drop(child);
    let mut bytes = Vec::new();
    stdout.take(OUTPUT_LIMIT + 1).read_to_end(&mut bytes)?;
    validate(status, bytes)
}
#[cfg(windows)]
pub(crate) fn execute(command: &[std::ffi::OsString], require_ordinary: bool) -> Result<i32> {
    use super::access_token::{current_token, is_elevated};
    use std::os::windows::process::CommandExt as _;
    use windows::Win32::CREATE_NO_WINDOW;
    if require_ordinary {
        ensure!(
            !is_elevated(&current_token()?)?,
            "ordinary shell probe still has administrator permissions"
        );
    }
    let (program, arguments) = command
        .split_first()
        .context("shell probe has no executable")?;
    std::process::Command::new(program)
        .args(arguments)
        .creation_flags(CREATE_NO_WINDOW.cast_unsigned())
        .status()
        .context("failed to start shell probe")?
        .code()
        .context("shell probe did not provide an exit code")
}
#[cfg(unix)]
pub(crate) fn query(
    program: &Path,
    arguments: &[&str],
    cwd: &Path,
    environment: &EnvironmentSnapshot,
    identity: &ShellIdentity,
) -> Result<Vec<u8>> {
    use std::ffi::OsStr;
    use std::process::Stdio;
    use tokio::io::AsyncReadExt as _;
    let argv = identity.command_line(
        core::iter::once(program.as_os_str()).chain(arguments.iter().map(OsStr::new)),
    )?;
    let (executable, parameters) = argv
        .split_first()
        .context("shell probe has no executable")?;
    tokio::runtime::Handle::current().block_on(async {
        let mut child = tokio::process::Command::new(executable)
            .args(parameters)
            .current_dir(cwd)
            .env_clear()
            .envs(environment.variables())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .context("shell probe has no output pipe")?;
        let mut limited = stdout.take(OUTPUT_LIMIT + 1);
        let mut bytes = Vec::new();
        let (status, _read) = tokio::time::timeout(TIMEOUT, async {
            tokio::try_join!(child.wait(), limited.read_to_end(&mut bytes))
        })
        .await
        .context("shell probe exceeded its time limit")??;
        validate(status, bytes)
    })
}
fn validate(status: ExitStatus, output: Vec<u8>) -> Result<Vec<u8>> {
    ensure!(status.success(), "shell probe failed with {status}");
    ensure!(
        u64::try_from(output.len())? <= OUTPUT_LIMIT,
        "shell probe exceeded its output limit"
    );
    Ok(output)
}
