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
