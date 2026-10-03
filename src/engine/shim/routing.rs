use crate::shell::ShellChoice;
use std::ffi::OsString;
use std::io::{IsTerminal as _, stdin};
#[derive(Debug, PartialEq, Eq)]
pub(super) enum LaunchRoute {
    ManagedSession,
    NativeProcess,
}
impl LaunchRoute {
    pub(super) fn detect(choice: ShellChoice, arguments: &[OsString]) -> Self {
        Self::classify(choice, arguments, stdin().is_terminal())
    }
    fn classify(choice: ShellChoice, arguments: &[OsString], terminal_input: bool) -> Self {
        if terminal_input && choice.interactive_arguments(arguments) {
            Self::ManagedSession
        } else {
            Self::NativeProcess
        }
    }
}
pub(super) fn load_profile(arguments: &[OsString]) -> anyhow::Result<bool> {
    let inherited = match std::env::var(crate::shell::shims::LOAD_PROFILE_ENV)?.as_str() {
        "1" => true,
        "0" => false,
        value => anyhow::bail!("invalid Shell Profile policy: {value}"),
    };
    let disabled = arguments.iter().any(|argument| {
        argument.to_str().is_some_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "-noprofile"
                    | "--noprofile"
                    | "--norc"
                    | "--no-config-file"
                    | "--no-rcs"
                    | "-f"
                    | "/d"
            )
        })
    });
    Ok(inherited && !disabled)
}
