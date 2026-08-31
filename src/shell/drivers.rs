mod bun;
mod command_prompt;
mod nushell;
mod powershell;
mod python;
mod unix_shell;
use super::ShellChoice;
use crate::contract::DISPATCHER_COMMAND;
use alloc::borrow::Cow;
use anyhow::{Result, bail};
use std::ffi::OsString;
use std::path::Path;
#[derive(Clone, Copy)]
pub(crate) struct StartupContext<'value> {
    pub(crate) cwd: &'value Path,
    pub(crate) startup_directory: &'value Path,
    pub(crate) ready_file: &'value Path,
}
pub(crate) struct ShellInvocation {
    line: String,
    terminator: InvocationTerminator,
}
#[derive(Clone, Copy)]
pub(crate) enum InvocationTerminator {
    CarriageReturn,
    LineFeed,
    CarriageReturnLineFeed,
}
pub(crate) struct DriverStartup {
    pub(crate) args: Vec<String>,
    pub(crate) env: Vec<(String, String)>,
}
impl ShellInvocation {
    pub(crate) fn new(line: String, terminator: InvocationTerminator) -> Result<Self> {
        if line.contains(['\r', '\n']) {
            bail!("shell invocation line must not contain line breaks");
        }
        Ok(Self { line, terminator })
    }
    pub(crate) fn into_bytes(mut self) -> Vec<u8> {
        self.line.push_str(self.terminator.as_str());
        self.line.into_bytes()
    }
}
impl InvocationTerminator {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CarriageReturn => "\r",
            Self::LineFeed => "\n",
            Self::CarriageReturnLineFeed => "\r\n",
        }
    }
}
pub(crate) fn startup(choice: ShellChoice, context: StartupContext<'_>) -> Result<DriverStartup> {
    match choice {
        ShellChoice::PowerShell => powershell::startup(context),
        ShellChoice::Bash => unix_shell::bash_startup(context),
        ShellChoice::NuShell => nushell::startup(context),
        ShellChoice::Zsh => unix_shell::zsh_startup(context),
        ShellChoice::Cmd => command_prompt::startup(context),
        ShellChoice::Bun => bun::startup(context),
        ShellChoice::Python => python::startup(context),
    }
}
pub(crate) fn invocation(choice: ShellChoice) -> Result<Option<ShellInvocation>> {
    let line = match choice {
        ShellChoice::Bun => return Ok(None),
        ShellChoice::Python => "_functerm_dispatch()",
        ShellChoice::PowerShell
        | ShellChoice::Bash
        | ShellChoice::NuShell
        | ShellChoice::Zsh
        | ShellChoice::Cmd => DISPATCHER_COMMAND,
    };
    ShellInvocation::new(line.to_owned(), invocation_terminator(choice)).map(Some)
}
pub(crate) fn command_script(choice: ShellChoice, command: &str) -> String {
    match choice {
        ShellChoice::PowerShell => powershell::command_script(command),
        ShellChoice::Bash
        | ShellChoice::NuShell
        | ShellChoice::Zsh
        | ShellChoice::Cmd
        | ShellChoice::Bun
        | ShellChoice::Python => command.to_owned(),
    }
}
pub(crate) fn keyboard_bytes(choice: ShellChoice, bytes: &[u8]) -> Cow<'_, [u8]> {
    match choice {
        ShellChoice::PowerShell => powershell::keyboard_bytes(bytes),
        ShellChoice::Bash
        | ShellChoice::NuShell
        | ShellChoice::Zsh
        | ShellChoice::Cmd
        | ShellChoice::Bun
        | ShellChoice::Python => Cow::Borrowed(bytes),
    }
}
pub(crate) fn interactive_arguments(choice: ShellChoice, arguments: &[OsString]) -> bool {
    match choice {
        ShellChoice::PowerShell => powershell::interactive_arguments(arguments),
        ShellChoice::Bash | ShellChoice::Zsh => unix_shell::interactive_arguments(arguments),
        ShellChoice::NuShell => nushell::interactive_arguments(arguments),
        ShellChoice::Cmd => command_prompt::interactive_arguments(arguments),
        ShellChoice::Bun => bun::interactive_arguments(arguments),
        ShellChoice::Python => python::interactive_arguments(arguments),
    }
}
const fn invocation_terminator(choice: ShellChoice) -> InvocationTerminator {
    match choice {
        ShellChoice::PowerShell => InvocationTerminator::CarriageReturn,
        ShellChoice::Python => platform_carriage_return(),
        ShellChoice::NuShell => platform_line_ending(),
        ShellChoice::Cmd => InvocationTerminator::CarriageReturnLineFeed,
        ShellChoice::Bash | ShellChoice::Zsh | ShellChoice::Bun => InvocationTerminator::LineFeed,
    }
}
#[cfg(windows)]
const fn platform_carriage_return() -> InvocationTerminator {
    InvocationTerminator::CarriageReturn
}
#[cfg(not(windows))]
const fn platform_carriage_return() -> InvocationTerminator {
    InvocationTerminator::LineFeed
}
#[cfg(windows)]
const fn platform_line_ending() -> InvocationTerminator {
    InvocationTerminator::CarriageReturnLineFeed
}
#[cfg(not(windows))]
const fn platform_line_ending() -> InvocationTerminator {
    InvocationTerminator::LineFeed
}
pub(crate) fn from_shim_name(value: &str) -> Option<ShellChoice> {
    let normalized = value.to_ascii_lowercase();
    ShellChoice::all().iter().copied().find(|choice| {
        choice
            .shim_executable_names()
            .contains(&normalized.as_str())
    })
}
pub(super) fn os_strings_lower(arguments: &[OsString]) -> Option<Vec<String>> {
    arguments
        .iter()
        .map(|argument| argument.to_str().map(str::to_ascii_lowercase))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::{InvocationTerminator, ShellInvocation};
    #[test]
    fn invocation_rejects_embedded_line_breaks() {
        for line in ["command\nnext", "command\rnext"] {
            let result =
                ShellInvocation::new(line.to_owned(), InvocationTerminator::CarriageReturn);
            assert!(
                result.is_err(),
                "invocation line with a line break should be rejected"
            );
        }
    }
}
