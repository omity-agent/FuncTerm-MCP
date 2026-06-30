use crate::config::Settings;
mod bash;
mod nushell;
mod powershell;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ShellChoice {
    PowerShell,
    Pwsh,
    Bash,
    NuShell,
}
pub(crate) struct ShellStartup {
    pub(crate) args: Vec<String>,
}
impl ShellChoice {
    pub(crate) fn executable(self, settings: &Settings) -> &str {
        match self {
            Self::PowerShell => &settings.windows_powershell,
            Self::Pwsh => &settings.pwsh,
            Self::Bash => &settings.bash,
            Self::NuShell => &settings.nushell,
        }
    }
    pub(crate) fn startup(self, cwd: &Path, session_root: &Path) -> Result<ShellStartup> {
        let args = match self {
            Self::PowerShell | Self::Pwsh => powershell::startup_args(cwd),
            Self::Bash => bash::startup_args(cwd, session_root)?,
            Self::NuShell => nushell::startup_args(cwd),
        };
        Ok(ShellStartup { args })
    }
    pub(crate) fn invocation(self, command_id: &str, command: &str, directory: &Path) -> String {
        match self {
            Self::PowerShell | Self::Pwsh => powershell::invocation(command_id, command, directory),
            Self::Bash => bash::invocation(command_id, command, directory),
            Self::NuShell => nushell::invocation(command_id, command, directory),
        }
    }
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "powershell" | "windows_powershell" => Ok(Self::PowerShell),
            "pwsh" | "powershell_core" => Ok(Self::Pwsh),
            "bash" => Ok(Self::Bash),
            "nu" | "nushell" => Ok(Self::NuShell),
            other => bail!(
                "unsupported shell `{other}`; supported shells are powershell, pwsh, bash, and nu"
            ),
        }
    }
}
