mod command_prompt;
mod nushell;
mod powershell;
mod unix_shell;
use super::ShellChoice;
use crate::runtime::config::Settings;
use alloc::borrow::Cow;
use anyhow::{Result, bail};
use std::ffi::OsString;
use std::path::Path;
const PROMPT_END_SEQUENCE: &str = "\x1b]133;B\x1b\\";
#[derive(Clone, Copy)]
pub(crate) struct StartupContext<'value> {
    pub(crate) cwd: &'value Path,
    pub(crate) startup_directory: &'value Path,
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
    fn display_name(&self) -> &'static str;
    fn shim_executable_names(&self) -> &'static [&'static str];
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
static BASH: unix_shell::PosixDriver = unix_shell::PosixDriver::bash();
static NUSHELL: nushell::NuShellDriver = nushell::NuShellDriver;
static ZSH: unix_shell::PosixDriver = unix_shell::PosixDriver::zsh();
static CMD: command_prompt::CmdDriver = command_prompt::CmdDriver;
pub(crate) fn driver(choice: ShellChoice) -> &'static dyn ShellDriver {
    match choice {
        ShellChoice::PowerShell => &POWERSHELL,
        ShellChoice::Bash => &BASH,
        ShellChoice::NuShell => &NUSHELL,
        ShellChoice::Zsh => &ZSH,
        ShellChoice::Cmd => &CMD,
    }
}
pub(crate) fn from_canonical_name(value: &str) -> Result<ShellChoice> {
    for choice in ShellChoice::all() {
        let shell = driver(choice);
        if shell.id() == value {
            return Ok(shell.choice());
        }
    }
    bail!("unknown shell")
}
pub(crate) fn from_shim_name(value: &str) -> Option<ShellChoice> {
    let normalized = value.to_ascii_lowercase();
    ShellChoice::all().into_iter().find(|choice| {
        driver(*choice)
            .shim_executable_names()
            .contains(&normalized.as_str())
    })
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
