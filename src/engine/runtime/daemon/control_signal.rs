#[cfg(windows)]
use anyhow::Context as _;
use anyhow::Result;
#[cfg(windows)]
pub(super) fn enable_ctrl_c_for_descendants() -> Result<()> {
    use windows::Win32::SetConsoleCtrlHandler;
    unsafe { SetConsoleCtrlHandler(None, false) }
        .ok()
        .context("failed to restore Ctrl+C processing for daemon descendants")?;
    Ok(())
}
#[cfg(not(windows))]
pub(super) const fn enable_ctrl_c_for_descendants() -> Result<()> {
    Ok(())
}
