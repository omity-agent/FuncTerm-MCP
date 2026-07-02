use anyhow::{Context as _, Result};
use std::fs;
use std::process::{Command, Stdio};
#[cfg(unix)]
pub(super) fn attach_terminal_stdio(command: &mut Command) -> Result<()> {
    let input = fs::File::open("/dev/tty").context("failed to open terminal input")?;
    let output = fs::OpenOptions::new()
        .write(true)
        .open("/dev/tty")
        .context("failed to open terminal output")?;
    let error = output
        .try_clone()
        .context("failed to clone terminal output")?;
    command
        .stdin(Stdio::from(input))
        .stdout(Stdio::from(output))
        .stderr(Stdio::from(error));
    Ok(())
}
#[cfg(windows)]
pub(super) fn attach_terminal_stdio(command: &mut Command) -> Result<()> {
    let input = fs::File::open("CONIN$").context("failed to open console input")?;
    let output = fs::OpenOptions::new()
        .write(true)
        .open("CONOUT$")
        .context("failed to open console output")?;
    let error = output
        .try_clone()
        .context("failed to clone console output")?;
    command
        .stdin(Stdio::from(input))
        .stdout(Stdio::from(output))
        .stderr(Stdio::from(error));
    Ok(())
}
#[cfg(not(any(unix, windows)))]
pub(super) fn attach_terminal_stdio(_command: &mut Command) -> Result<()> {
    anyhow::bail!("interactive shell shims are not supported on this platform")
}
