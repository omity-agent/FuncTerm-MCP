use crate::runtime::config::Settings;
mod bash;
mod nushell;
mod powershell;
use anyhow::{Result, bail};
use std::path::Path;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShellChoice {
    PowerShell,
    Bash,
    NuShell,
}
pub(crate) struct ShellStartup {
    pub(crate) args: Vec<String>,
    pub(crate) ready_file: std::path::PathBuf,
}
impl ShellChoice {
    pub(crate) fn executable(self, settings: &Settings) -> Result<String> {
        match self {
            Self::PowerShell => select_available_executable(&settings.powershell),
            Self::Bash => Ok(settings.bash.clone()),
            Self::NuShell => Ok(settings.nushell.clone()),
        }
    }
    pub(crate) fn startup(self, cwd: &Path, session_root: &Path) -> Result<ShellStartup> {
        let ready_file = session_root.join("startup.ready");
        let args = match self {
            Self::PowerShell => powershell::startup_args(cwd, &ready_file),
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
            Self::PowerShell => powershell::invocation(command_id, command, directory, cwd),
            Self::Bash => bash::invocation(command_id, command, directory, cwd),
            Self::NuShell => nushell::invocation(command_id, command, directory, cwd),
        }
    }
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe" | "powershell_core"
            | "windows_powershell" => Ok(Self::PowerShell),
            "bash" | "bash.exe" => Ok(Self::Bash),
            "nu" | "nu.exe" | "nushell" | "nushell.exe" => Ok(Self::NuShell),
            other => bail!(
                "unsupported shell `{other}`; supported shells are powershell, bash, and nushell"
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
#[expect(
    clippy::inline_modules,
    reason = "Rust skill permits inline modules guarded by cfg(test)"
)]
mod tests {
    use super::ShellChoice;
    #[test]
    fn shell_aliases_parse_to_three_choices() {
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
    }
}
