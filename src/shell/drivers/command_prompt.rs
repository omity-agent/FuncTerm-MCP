use super::{DriverStartup, StartupContext, os_strings_lower};
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::{cmd_dispatcher, cmd_wrapper};
use anyhow::Result;
pub(super) fn startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let runner = context.startup_directory.join("cmd_run.bat");
    let dispatcher = context.startup_directory.join("f.cmd");
    let init = context.startup_directory.join("cmd_init.bat");
    fs_err::write(&runner, cmd_wrapper())?;
    fs_err::write(&dispatcher, cmd_dispatcher())?;
    fs_err::write(&init, initialization_script(context)?)?;
    Ok(DriverStartup {
        args: vec![
            "/D".to_owned(),
            "/Q".to_owned(),
            "/K".to_owned(),
            format!("call {}", quote::native_path(&init)?),
        ],
        env: Vec::new(),
    })
}
pub(super) fn interactive_arguments(arguments: &[std::ffi::OsString]) -> bool {
    let Some(values) = os_strings_lower(arguments) else {
        return false;
    };
    values
        .iter()
        .all(|value| matches!(value.as_str(), "/d" | "/q" | "/k"))
}
fn initialization_script(context: StartupContext<'_>) -> Result<String> {
    Ok(format!(
        "@echo off\r\nset {CURRENT_SHELL_ENV}=cmd\r\nset \"PATH=%FUNCTERM_SESSION_ROOT%\\startup;%PATH%\"\r\ndoskey /listsize=0 >nul 2>nul\r\ncd /d {}\r\ntype nul > {}\r\n",
        quote::cmd_string(&quote::native_path(context.cwd)?),
        quote::cmd_string(&quote::native_path(context.ready_file)?)
    ))
}
#[cfg(test)]
mod tests {
    use crate::shell::ShellChoice;
    #[test]
    fn invocation_uses_windows_line_ending() {
        let bytes = crate::shell::drivers::invocation(ShellChoice::Cmd)
            .unwrap()
            .unwrap()
            .into_bytes();
        assert_eq!(bytes, b"f\r\n");
    }
}
