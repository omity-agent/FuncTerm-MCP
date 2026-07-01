use crate::runtime::config::Settings;
mod bash;
mod nushell;
mod powershell;
use anyhow::{Result, bail};
use std::path::Path;
#[derive(Clone, Copy, Debug)]
pub(crate) enum ShellChoice {
    PowerShell,
    Pwsh,
    Bash,
    NuShell,
}
pub(crate) struct ShellStartup {
    pub(crate) args: Vec<String>,
    pub(crate) ready_file: std::path::PathBuf,
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
        let ready_file = session_root.join("startup.ready");
        let args = match self {
            Self::PowerShell | Self::Pwsh => powershell::startup_args(cwd, &ready_file),
            Self::Bash => bash::startup_args(cwd, session_root, &ready_file)?,
            Self::NuShell => nushell::startup_args(cwd, &ready_file),
        };
        Ok(ShellStartup { args, ready_file })
    }
    pub(crate) fn invocation(
        self,
        command_id: &str,
        command: &str,
        directory: &Path,
        cwd: &Path,
    ) -> String {
        match self {
            Self::PowerShell | Self::Pwsh => {
                powershell::invocation(command_id, command, directory, cwd)
            }
            Self::Bash => bash::invocation(command_id, command, directory, cwd),
            Self::NuShell => nushell::invocation(command_id, command, directory, cwd),
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
