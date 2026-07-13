use anyhow::{Context as _, Result};
use std::fs;
use std::process::{Command, Stdio};
#[cfg(unix)]
pub(super) fn attach_terminal_stdio(command: &mut Command) -> Result<()> {
    let input = fs::File::open("/dev/tty").context("failed to open terminal input")?;
    let output = terminal_output()?;
    let error = output
        .try_clone()
        .context("failed to clone terminal output")?;
    command
        .stdin(Stdio::from(input))
        .stdout(Stdio::from(output))
        .stderr(Stdio::from(error));
    Ok(())
}
#[cfg(unix)]
pub(super) fn terminal_output() -> Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .open("/dev/tty")
        .context("failed to open terminal output")
}
#[cfg(windows)]
pub(super) fn attach_terminal_stdio(command: &mut Command) -> Result<()> {
    let input = fs::File::open("CONIN$").context("failed to open console input")?;
    let output = terminal_output()?;
    let error = output
        .try_clone()
        .context("failed to clone console output")?;
    command
        .stdin(Stdio::from(input))
        .stdout(Stdio::from(output))
        .stderr(Stdio::from(error));
    Ok(())
}
#[cfg(windows)]
pub(super) fn terminal_output() -> Result<fs::File> {
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
    use windows_sys::Win32::System::Console::{
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, SetConsoleMode,
    };
    let handle = output.as_raw_handle();
    let mut mode = 0_u32;
    if unsafe { GetConsoleMode(handle, &raw mut mode) } == 0_i32 {
        return Err(std::io::Error::last_os_error()).context("failed to read console output mode");
    }
    if unsafe { SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) } == 0_i32 {
        return Err(std::io::Error::last_os_error())
            .context("failed to enable virtual terminal output");
    }
    Ok(())
}
#[cfg(not(any(unix, windows)))]
pub(super) fn attach_terminal_stdio(_command: &mut Command) -> Result<()> {
    anyhow::bail!("interactive shell shims are not supported on this platform")
}
#[cfg(not(any(unix, windows)))]
pub(super) fn terminal_output() -> Result<fs::File> {
    anyhow::bail!("interactive shell shims are not supported on this platform")
}
