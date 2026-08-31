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
            self.executable_candidates(settings),
            environment,
            cwd,
        )
    }
    pub(crate) fn startup(self, cwd: &Path, session_root: &Path) -> Result<ShellStartup> {
        let state_directory = session_root.join("state");
        let startup_directory = session_root.join("startup");
        fs_err::create_dir_all(&state_directory)?;
        fs_err::create_dir_all(&startup_directory)?;
        let ready_file = state_directory.join("ready");
        let startup = drivers::startup(
            self,
            drivers::StartupContext {
                cwd,
                startup_directory: &startup_directory,
                ready_file: &ready_file,
            },
        )?;
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
    pub(crate) fn invocation(self) -> Result<Option<drivers::ShellInvocation>> {
        drivers::invocation(self)
    }
    pub(crate) fn command_script(self, command: &str) -> String {
        drivers::command_script(self, command)
    }
    pub(crate) fn keyboard_bytes(self, bytes: &[u8]) -> Cow<'_, [u8]> {
        drivers::keyboard_bytes(self, bytes)
    }
    pub(crate) fn from_canonical_name(value: &str) -> Result<Self> {
        let parsed = value
            .parse::<Self>()
            .map_err(|error| anyhow::anyhow!("unknown shell: {error}"));
        parsed.with_context(|| {
            format!(
                "unsupported shell `{value}`; supported shells are {}",
                <Self as strum::VariantNames>::VARIANTS.join(", ")
            )
        })
    }
    pub(crate) fn from_shim_name(value: &str) -> Option<Self> {
        drivers::from_shim_name(value)
    }
    pub(crate) const fn all() -> &'static [Self] {
        <Self as strum::VariantArray>::VARIANTS
    }
    pub(crate) fn interactive_arguments(self, arguments: &[std::ffi::OsString]) -> bool {
        drivers::interactive_arguments(self, arguments)
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
            <ShellChoice as strum::VariantNames>::VARIANTS,
            ["powershell", "bash", "nu", "zsh", "cmd", "bun", "python"]
        );
        for &choice in ShellChoice::all() {
            assert_eq!(
                ShellChoice::from_canonical_name(choice.canonical_name()).unwrap(),
                choice
            );
        }
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
        assert_eq!(
            ShellChoice::from_shim_name("bun.exe"),
            Some(ShellChoice::Bun)
        );
    }
    #[test]
    fn starting_shell_does_not_accept_executable_aliases() {
        assert_rejected_starting_shell("pwsh");
        assert_rejected_starting_shell("powershell.exe");
        assert_rejected_starting_shell("bash.exe");
        assert_rejected_starting_shell("nushell");
        assert_rejected_starting_shell("cmd.exe");
        assert_rejected_starting_shell("bun.exe");
    }
}
