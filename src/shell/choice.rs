use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum ShellChoice {
    PowerShell,
    Bash,
    NuShell,
    Zsh,
    Cmd,
}
