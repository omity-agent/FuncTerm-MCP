use super::{DriverStartup, InvocationTerminator, ShellDriver, StartupContext, os_strings_lower};
use crate::runtime::config::Settings;
use crate::shell::ShellChoice;
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::{cmd_dispatcher, cmd_wrapper};
use anyhow::{Context as _, Result};
pub(crate) struct CmdDriver;
impl ShellDriver for CmdDriver {
    fn choice(&self) -> ShellChoice {
        ShellChoice::Cmd
    }
    fn id(&self) -> &'static str {
        "cmd"
    }
    fn display_name(&self) -> &'static str {
        "Windows CMD"
    }
    fn shim_executable_names(&self) -> &'static [&'static str] {
        &["cmd", "cmd.exe"]
    }
    fn shim_env_name(&self) -> &'static str {
        "FUNCTERM_REAL_CMD"
    }
    fn executable_candidates(&self, settings: &Settings) -> Result<Vec<String>> {
        Ok(vec![settings.cmd.clone()])
    }
    fn startup(&self, context: StartupContext<'_>) -> Result<DriverStartup> {
        let runner = context.startup_directory.join("cmd_run.bat");
        let dispatcher = context.startup_directory.join("f.cmd");
        let init = context.startup_directory.join("cmd_init.bat");
        std::fs::write(&runner, cmd_wrapper()).context("failed to write Cmd command wrapper")?;
        std::fs::write(&dispatcher, cmd_dispatcher())
            .context("failed to write Cmd command dispatcher")?;
        std::fs::write(&init, initialization_script(context)?)
            .context("failed to write Cmd initialization script")?;
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
    fn invocation_terminator(&self) -> InvocationTerminator {
        InvocationTerminator::CarriageReturnLineFeed
    }
    fn interactive_arguments(&self, arguments: &[std::ffi::OsString]) -> bool {
        let Some(values) = os_strings_lower(arguments) else {
            return false;
        };
        values
            .iter()
            .all(|value| matches!(value.as_str(), "/d" | "/q" | "/k"))
    }
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
    use super::CmdDriver;
    use crate::shell::drivers::ShellDriver as _;
    #[test]
    fn invocation_uses_windows_line_ending() {
        let bytes = CmdDriver.invocation().unwrap().into_bytes();
        assert_eq!(bytes, b"f\r\n");
    }
}
