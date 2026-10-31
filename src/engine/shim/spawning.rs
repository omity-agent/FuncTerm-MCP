use super::{real_executable, stdio};
use crate::shell::{ShellChoice, ShellStartup};
use anyhow::{Context as _, Result};
#[cfg(windows)]
use std::path::Path;
use std::process::{Child, Command};
pub(super) fn spawn_shell(choice: ShellChoice, startup: ShellStartup) -> Result<Child> {
    let executable = real_executable(choice)?;
    let mut command = process_command(&executable, &startup.args)?;
    command.envs(startup.env);
    stdio::attach_session_stdio(&mut command)?;
    command
        .spawn()
        .with_context(|| format!("failed to spawn {}", choice.canonical_name()))
}
fn process_command(executable: &str, arguments: &[String]) -> Result<Command> {
    #[cfg(windows)]
    if Path::new(executable)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        })
    {
        let command_line = core::iter::once(format!(
            "call {}",
            crate::shell::quote::cmd_string(executable)
        ))
        .chain(
            arguments
                .iter()
                .map(|argument| crate::shell::quote::cmd_string(argument)),
        )
        .collect::<Vec<_>>()
        .join(" ");
        let mut command = Command::new(
            std::env::var_os("ComSpec").context("ComSpec is required to launch a batch shell")?,
        );
        command.args(["/D", "/Q", "/S", "/C", &command_line]);
        return Ok(command);
    }
    let mut command = Command::new(executable);
    command.args(arguments);
    Ok(command)
}
