#[cfg(windows)]
use anyhow::Context as _;
use anyhow::Result;
#[cfg(windows)]
pub(super) fn enable_ctrl_c_for_descendants() -> Result<()> {
    use windows::Win32::System::Console::SetConsoleCtrlHandler;
    unsafe { SetConsoleCtrlHandler(None, false) }
        .map_err(anyhow::Error::from)
        .context("failed to restore Ctrl+C processing for daemon descendants")?;
    Ok(())
}
#[cfg(not(windows))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the shared daemon interface propagates failures from the Windows implementation"
)]
pub(super) const fn enable_ctrl_c_for_descendants() -> Result<()> {
    Ok(())
}
