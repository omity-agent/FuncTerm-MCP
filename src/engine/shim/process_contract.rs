use std::ffi::OsString;
use std::process::{Command, ExitCode};
#[cfg(windows)]
pub(crate) fn process_exit(code: i32) -> ExitCode {
    use std::os::windows::process::ExitCodeExt as _;
    ExitCode::from_raw(code.cast_unsigned())
}
#[cfg(unix)]
pub(crate) fn process_exit(code: i32) -> ExitCode {
    let [low, ..] = code.to_le_bytes();
    ExitCode::from(low)
}
#[cfg(unix)]
pub(super) fn command(executable: &str, arguments: &[OsString]) -> Command {
    let mut command = Command::new(executable);
    command.args(arguments);
    command
}
#[cfg(windows)]
pub(super) fn command(executable: &str, _arguments: &[OsString]) -> Command {
    use std::os::windows::{ffi::OsStringExt as _, process::CommandExt as _};
    let raw = unsafe { windows::Win32::GetCommandLineW() };
    let wide = unsafe { raw.as_wide() };
    let mut quoted = false;
    let end = wide
        .iter()
        .position(|&character| {
            if character == u16::from(b'"') {
                quoted = !quoted;
            }
            !quoted && matches!(character, 0x20 | 0x09)
        })
        .unwrap_or(wide.len());
    let (_, tail) = wide.split_at(end);
    let mut command = Command::new(executable);
    command.raw_arg(OsString::from_wide(tail));
    command
}
