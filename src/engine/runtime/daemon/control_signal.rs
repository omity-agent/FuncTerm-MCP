use anyhow::{Context as _, Result};
#[cfg(windows)]
pub(super) fn enable_ctrl_c_for_descendants() -> Result<()> {
    use windows::Win32::System::Console::SetConsoleCtrlHandler;
    unsafe { SetConsoleCtrlHandler(None, false) }
        .map_err(anyhow::Error::from)
        .context("failed to restore Ctrl+C processing for daemon descendants")?;
    Ok(())
}
#[cfg(not(windows))]
pub(super) const fn enable_ctrl_c_for_descendants() -> Result<()> {
    Ok(())
}
