mod cmd;
mod nushell;
mod posix;
mod powershell;
use super::ShellChoice;
use crate::runtime::config::Settings;
use alloc::borrow::Cow;
use anyhow::{Result, bail};
use std::ffi::OsString;
use std::path::Path;
#[derive(Clone, Copy)]
pub(crate) struct StartupContext<'value> {
    pub(crate) cwd: &'value Path,
    pub(crate) session_root: &'value Path,
    pub(crate) ready_file: &'value Path,
}
pub(crate) struct InvocationContext<'value> {
    pub(crate) command_id: &'value str,
    pub(crate) directory: &'value Path,
    pub(crate) cwd: &'value Path,
}
pub(crate) struct DriverStartup {
    pub(crate) args: Vec<String>,
    pub(crate) env: Vec<(String, String)>,
}
pub(crate) trait ShellDriver {
    fn choice(&self) -> ShellChoice;
    fn id(&self) -> &'static str;
    fn aliases(&self) -> &'static [&'static str];
    fn shim_env_name(&self) -> &'static str;
    fn executable_candidates(&self, settings: &Settings) -> Result<Vec<String>>;
    fn startup(&self, context: StartupContext<'_>) -> Result<DriverStartup>;
    fn invocation(&self, context: InvocationContext<'_>) -> Result<String>;
    fn keyboard_bytes<'bytes>(&self, bytes: &'bytes [u8]) -> Cow<'bytes, [u8]> {
        Cow::Borrowed(bytes)
    }
    fn interactive_arguments(&self, arguments: &[OsString]) -> bool;
}
static POWERSHELL: powershell::PowerShellDriver = powershell::PowerShellDriver;
static BASH: posix::PosixDriver = posix::PosixDriver::bash();
static NUSHELL: nushell::NuShellDriver = nushell::NuShellDriver;
static ZSH: posix::PosixDriver = posix::PosixDriver::zsh();
static CMD: cmd::CmdDriver = cmd::CmdDriver;
pub(crate) fn driver(choice: ShellChoice) -> &'static dyn ShellDriver {
    match choice {
        ShellChoice::PowerShell => &POWERSHELL,
        ShellChoice::Bash => &BASH,
        ShellChoice::NuShell => &NUSHELL,
        ShellChoice::Zsh => &ZSH,
        ShellChoice::Cmd => &CMD,
    }
}
pub(crate) fn parse(value: &str) -> Result<ShellChoice> {
    let normalized = value.to_ascii_lowercase();
    for choice in ShellChoice::all() {
        let shell = driver(choice);
        if shell.id() == normalized || shell.aliases().contains(&normalized.as_str()) {
            return Ok(shell.choice());
        }
    }
    bail!("unknown shell")
}
pub(crate) fn supported_shells() -> String {
    ShellChoice::all()
        .into_iter()
        .map(|choice| driver(choice).id())
        .collect::<Vec<_>>()
        .join(", ")
}
pub(super) fn os_strings_lower(arguments: &[OsString]) -> Option<Vec<String>> {
    arguments
        .iter()
        .map(|argument| argument.to_str().map(str::to_ascii_lowercase))
        .collect()
}
