use anyhow::{Context as _, Result};
use std::fs;
use std::process::{Command, Stdio};
#[cfg(any(unix, windows))]
pub(super) fn attach_session_stdio(command: &mut Command) -> Result<()> {
    let output = terminal_output()?;
    let error = output
        .try_clone()
        .context("failed to clone terminal output")?;
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::from(output))
        .stderr(Stdio::from(error));
    Ok(())
}
#[cfg(unix)]
pub(crate) fn terminal_output() -> Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .open("/dev/tty")
        .context("failed to open terminal output")
}
#[cfg(windows)]
pub(crate) fn terminal_output() -> Result<fs::File> {
    let output = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("CONOUT$")
        .context("failed to open console output")?;
    enable_virtual_terminal_output(&output)?;
    Ok(output)
}
#[cfg(windows)]
fn enable_virtual_terminal_output(output: &fs::File) -> Result<()> {
    use std::os::windows::io::AsRawHandle as _;
    use windows::Win32::{ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, SetConsoleMode};
    let handle = output.as_raw_handle();
    let mut mode = 0;
    unsafe { GetConsoleMode(handle, &raw mut mode) }
        .ok()
        .context("failed to read console output mode")?;
    let virtual_terminal_mode = mode | u32::try_from(ENABLE_VIRTUAL_TERMINAL_PROCESSING)?;
    unsafe { SetConsoleMode(handle, virtual_terminal_mode) }
        .ok()
        .context("failed to enable virtual terminal output")?;
    Ok(())
}
#[cfg(not(any(unix, windows)))]
pub(super) fn attach_session_stdio(_command: &mut Command) -> Result<()> {
    anyhow::bail!("interactive shell shims are not supported on this platform")
}
#[cfg(not(any(unix, windows)))]
pub(crate) fn terminal_output() -> Result<fs::File> {
    anyhow::bail!("interactive shell shims are not supported on this platform")
}
