use super::{DriverStartup, InvocationContext, ShellDriver, StartupContext, os_strings_lower};
use crate::contract::POSIX_COMMAND_FUNCTION;
use crate::runtime::config::Settings;
use crate::shell::ShellChoice;
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::nushell_wrapper;
use anyhow::Result;
pub(crate) struct NuShellDriver;
impl ShellDriver for NuShellDriver {
    fn choice(&self) -> ShellChoice {
        ShellChoice::NuShell
    }
    fn id(&self) -> &'static str {
        "nu"
    }
    fn display_name(&self) -> &'static str {
        "NuShell"
    }
    fn shim_executable_names(&self) -> &'static [&'static str] {
        &["nu", "nu.exe", "nushell", "nushell.exe"]
    }
    fn shim_env_name(&self) -> &'static str {
        "FUNCTERM_REAL_NUSHELL"
    }
    fn executable_candidates(&self, settings: &Settings) -> Result<Vec<String>> {
        Ok(vec![settings.nushell.clone()])
    }
    fn startup(&self, context: StartupContext<'_>) -> Result<DriverStartup> {
        Ok(DriverStartup {
            args: vec![
                "--no-config-file".to_owned(),
                "--no-history".to_owned(),
                "--execute".to_owned(),
                initialization_script(context)?,
            ],
            env: Vec::new(),
        })
    }
    fn invocation(&self, context: InvocationContext<'_>) -> Result<String> {
        Ok(format!(
            "{POSIX_COMMAND_FUNCTION} {} {} {}{}",
            quote::nushell_string(context.command_id),
            quote::nushell_path(context.directory)?,
            quote::nushell_path(context.cwd)?,
            invocation_line_ending()
        ))
    }
    fn interactive_arguments(&self, arguments: &[std::ffi::OsString]) -> bool {
        let Some(values) = os_strings_lower(arguments) else {
            return false;
        };
        values.iter().all(|value| {
            matches!(
                value.as_str(),
                "--login" | "--no-config-file" | "--no-history"
            )
        })
    }
}
#[cfg(windows)]
const fn invocation_line_ending() -> &'static str {
    "\r\n"
}
#[cfg(not(windows))]
const fn invocation_line_ending() -> &'static str {
    "\n"
}
fn initialization_script(context: StartupContext<'_>) -> Result<String> {
    Ok(format!(
        "$env.{CURRENT_SHELL_ENV} = 'nu'\n{}\ncd {}\n'' | save --force --raw {}\n",
        nushell_wrapper(),
        quote::nushell_path(context.cwd)?,
        quote::nushell_path(context.ready_file)?
    ))
}
