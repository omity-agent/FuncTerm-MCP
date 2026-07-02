use crate::runtime::config::Settings;
mod bash;
mod nushell;
mod posix;
mod powershell;
mod zsh;
use alloc::borrow::Cow;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum ShellChoice {
    PowerShell,
    Bash,
    NuShell,
    Zsh,
}
pub(crate) struct ShellStartup {
    pub(crate) args: Vec<String>,
    pub(crate) env: Vec<(String, String)>,
    pub(crate) ready_file: std::path::PathBuf,
}
impl ShellChoice {
    pub(crate) fn executable(self, settings: &Settings) -> Result<String> {
        match self {
            Self::PowerShell => select_available_executable(&settings.powershell),
            Self::Bash => Ok(settings.bash.clone()),
            Self::NuShell => Ok(settings.nushell.clone()),
            Self::Zsh => Ok(settings.zsh.clone()),
        }
    }
    pub(crate) fn startup(self, cwd: &Path, session_root: &Path) -> Result<ShellStartup> {
        let ready_file = session_root.join("startup.ready");
        let (args, env) = match self {
            Self::PowerShell => (powershell::startup_args(cwd, &ready_file), Vec::new()),
            Self::Bash => (
                bash::startup_args(cwd, session_root, &ready_file)?,
                Vec::new(),
            ),
            Self::NuShell => (nushell::startup_args(cwd, &ready_file), Vec::new()),
            Self::Zsh => zsh::startup(cwd, session_root, &ready_file)?,
        };
        Ok(ShellStartup {
            args,
            env,
            ready_file,
        })
    }
    pub(crate) fn invocation(self, command_id: &str, directory: &Path, cwd: &Path) -> String {
        match self {
            Self::PowerShell => powershell::invocation(command_id, directory, cwd),
            Self::Bash => bash::invocation(command_id, directory, cwd),
            Self::NuShell => nushell::invocation(command_id, directory, cwd),
            Self::Zsh => zsh::invocation(command_id, directory, cwd),
        }
    }
    pub(crate) fn keyboard_bytes(self, bytes: &[u8]) -> Cow<'_, [u8]> {
        match self {
            Self::PowerShell => powershell::keyboard_bytes(bytes),
            Self::Bash | Self::NuShell | Self::Zsh => Cow::Borrowed(bytes),
        }
    }
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe" | "powershell_core"
            | "windows_powershell" => Ok(Self::PowerShell),
            "bash" | "bash.exe" => Ok(Self::Bash),
            "nu" | "nu.exe" | "nushell" | "nushell.exe" => Ok(Self::NuShell),
            "zsh" => Ok(Self::Zsh),
            other => bail!(
                "unsupported shell `{other}`; supported shells are powershell, bash, nushell, and zsh"
            ),
        }
    }
}
fn select_available_executable(candidates: &[String]) -> Result<String> {
    for candidate in candidates {
        if which::which(candidate).is_ok() {
            return Ok(candidate.clone());
        }
    }
    bail!(
        "none of the configured PowerShell executables are available: {}",
        candidates.join(", ")
    )
}
#[cfg(test)]
mod tests {
    use super::ShellChoice;
    #[test]
    fn shell_aliases_parse_to_supported_choices() {
        assert_eq!(
            ShellChoice::parse("powershell.exe").unwrap(),
            ShellChoice::PowerShell
        );
        assert_eq!(ShellChoice::parse("pwsh").unwrap(), ShellChoice::PowerShell);
        assert_eq!(ShellChoice::parse("bash.exe").unwrap(), ShellChoice::Bash);
        assert_eq!(ShellChoice::parse("nu").unwrap(), ShellChoice::NuShell);
        assert_eq!(
            ShellChoice::parse("nushell.exe").unwrap(),
            ShellChoice::NuShell
        );
        assert_eq!(ShellChoice::parse("zsh").unwrap(), ShellChoice::Zsh);
    }
}
