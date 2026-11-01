use anyhow::{Context as _, Result};
use std::io::Write;
use std::path::Path;
pub(super) fn restore(output: &mut impl Write, directory: &Path, stdio: bool) -> Result<()> {
    let title = std::fs::read_to_string(directory.join(crate::contract::TITLE_FILE))
        .context("failed to read command title")?;
    crate::contract::validate_window_title(&title)?;
    if stdio {
        return output
            .write_all(crate::contract::window_title_sequence(&title)?.as_bytes())
            .context("failed to restore terminal title");
    }
    apply(output, &title)
}
#[cfg(windows)]
fn apply(_output: &mut impl Write, title: &str) -> Result<()> {
    use windows::Win32::SetConsoleTitleW;
    use windows::core::PCWSTR;
    let wide_title: Vec<u16> = title.encode_utf16().chain(core::iter::once(0)).collect();
    unsafe { SetConsoleTitleW(PCWSTR(wide_title.as_ptr())) }
        .ok()
        .context("failed to restore terminal title")
}
#[cfg(unix)]
fn apply(output: &mut impl Write, title: &str) -> Result<()> {
    output
        .write_all(crate::contract::window_title_sequence(title)?.as_bytes())
        .context("failed to restore terminal title")
}
