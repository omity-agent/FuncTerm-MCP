use crate::runtime::config::Settings;
mod choice;
mod drivers;
mod executable;
pub mod quote;
pub(crate) mod shims;
mod wrappers;
use alloc::borrow::Cow;
use anyhow::{Context as _, Result};
pub(crate) use choice::ShellChoice;
use std::ffi::OsString;
use std::path::Path;
pub(crate) struct ShellStartup {
    pub(crate) args: Vec<String>,
    pub(crate) env: Vec<(OsString, OsString)>,
    pub(crate) ready_file: std::path::PathBuf,
}
impl ShellChoice {
    pub(crate) fn executable(
        self,
        settings: &Settings,
        environment: &crate::runtime::protocol::EnvironmentSnapshot,
        cwd: &Path,
    ) -> Result<String> {
        crate::text::path_text(
            &self.executable_path(settings, environment, cwd)?,
            "executable path",
        )
    }
    pub(crate) fn executable_path(
        self,
        settings: &Settings,
        environment: &crate::runtime::protocol::EnvironmentSnapshot,
        cwd: &Path,
    ) -> Result<std::path::PathBuf> {
        executable::select_available_executable(
            self,
            &self.driver().executable_candidates(settings)?,
            environment,
            cwd,
        )
    }
    pub(crate) fn startup(self, cwd: &Path, session_root: &Path) -> Result<ShellStartup> {
        let state_directory = session_root.join("state");
        let startup_directory = session_root.join("startup");
        std::fs::create_dir_all(&state_directory).with_context(|| {
            format!(
                "failed to create shell session state directory {}",
                state_directory.display()
            )
        })?;
        std::fs::create_dir_all(&startup_directory).with_context(|| {
            format!(
                "failed to create shell startup directory {}",
                startup_directory.display()
            )
        })?;
        let ready_file = state_directory.join("ready");
        let startup = self.driver().startup(drivers::StartupContext {
            cwd,
            startup_directory: &startup_directory,
            ready_file: &ready_file,
        })?;
        let args = startup.args;
        let env = startup
            .env
            .into_iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value)))
            .collect();
        Ok(ShellStartup {
            args,
            env,
            ready_file,
        })
    }
    pub(crate) fn invocation(
        self,
        command_id: &str,
        directory: &Path,
        cwd: &Path,
    ) -> Result<String> {
        self.driver().invocation(drivers::InvocationContext {
            command_id,
            directory,
            cwd,
        })
    }
    pub(crate) fn keyboard_bytes(self, bytes: &[u8]) -> Cow<'_, [u8]> {
        self.driver().keyboard_bytes(bytes)
    }
    pub(crate) fn from_canonical_name(value: &str) -> Result<Self> {
        drivers::from_canonical_name(value).with_context(|| {
            format!(
                "unsupported shell `{value}`; supported shells are {}",
                drivers::supported_shells()
            )
        })
    }
    pub(crate) fn from_shim_name(value: &str) -> Option<Self> {
        drivers::from_shim_name(value)
    }
    pub(crate) fn canonical_name(self) -> &'static str {
        self.driver().id()
    }
    pub(crate) fn display_name(self) -> &'static str {
        self.driver().display_name()
    }
    pub(crate) fn shim_env_name(self) -> &'static str {
        self.driver().shim_env_name()
    }
    pub(crate) fn shim_executable_names(self) -> &'static [&'static str] {
        self.driver().shim_executable_names()
    }
    pub(crate) const fn all() -> [Self; 5] {
        [
            Self::PowerShell,
            Self::Bash,
            Self::NuShell,
            Self::Zsh,
            Self::Cmd,
        ]
    }
    pub(crate) fn interactive_arguments(self, arguments: &[std::ffi::OsString]) -> bool {
        self.driver().interactive_arguments(arguments)
    }
    fn driver(self) -> &'static dyn drivers::ShellDriver {
        drivers::driver(self)
    }
}
#[cfg(test)]
mod tests {
    use super::ShellChoice;
    fn assert_rejected_starting_shell(value: &str) {
        if let Ok(choice) = ShellChoice::from_canonical_name(value) {
            panic!("{value} should not parse as starting shell, got {choice:?}");
        }
    }
    #[test]
    fn canonical_shell_names_parse_to_supported_choices() {
        assert_eq!(
            ShellChoice::from_canonical_name("powershell").unwrap(),
            ShellChoice::PowerShell
        );
        assert_eq!(
            ShellChoice::from_canonical_name("bash").unwrap(),
            ShellChoice::Bash
        );
        assert_eq!(
            ShellChoice::from_canonical_name("nu").unwrap(),
            ShellChoice::NuShell
        );
        assert_eq!(
            ShellChoice::from_canonical_name("zsh").unwrap(),
            ShellChoice::Zsh
        );
        assert_eq!(
            ShellChoice::from_canonical_name("cmd").unwrap(),
            ShellChoice::Cmd
        );
    }
    #[test]
    fn executable_aliases_only_parse_for_shim_invocation() {
        assert_eq!(
            ShellChoice::from_shim_name("powershell.exe"),
            Some(ShellChoice::PowerShell)
        );
        assert_eq!(
            ShellChoice::from_shim_name("pwsh"),
            Some(ShellChoice::PowerShell)
        );
        assert_eq!(
            ShellChoice::from_shim_name("bash.exe"),
            Some(ShellChoice::Bash)
        );
        assert_eq!(
            ShellChoice::from_shim_name("nushell.exe"),
            Some(ShellChoice::NuShell)
        );
        assert_eq!(
            ShellChoice::from_shim_name("cmd.exe"),
            Some(ShellChoice::Cmd)
        );
    }
    #[test]
    fn starting_shell_does_not_accept_executable_aliases() {
        assert_rejected_starting_shell("pwsh");
        assert_rejected_starting_shell("powershell.exe");
        assert_rejected_starting_shell("bash.exe");
        assert_rejected_starting_shell("nushell");
        assert_rejected_starting_shell("cmd.exe");
    }
}
