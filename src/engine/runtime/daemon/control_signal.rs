use anyhow::Result;
#[cfg(windows)]
pub(super) fn enable_ctrl_c_for_descendants() -> Result<()> {
    use windows_sys::Win32::Foundation::FALSE;
    use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
    let restored = unsafe { SetConsoleCtrlHandler(None, FALSE) };
    anyhow::ensure!(
        restored != FALSE,
        "failed to restore Ctrl+C processing for daemon descendants: {}",
        std::io::Error::last_os_error()
    );
    Ok(())
}
#[cfg(not(windows))]
pub(super) const fn enable_ctrl_c_for_descendants() -> Result<()> {
    Ok(())
}
