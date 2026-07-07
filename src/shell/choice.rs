use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize, schemars :: JsonSchema)]
pub(crate) enum ShellChoice {
    #[serde(rename = "powershell")]
    PowerShell,
    #[serde(rename = "bash")]
    Bash,
    #[serde(rename = "nu")]
    NuShell,
    #[serde(rename = "zsh")]
    Zsh,
    #[serde(rename = "cmd")]
    Cmd,
}
