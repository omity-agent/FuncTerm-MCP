use crate::engine::privileges::ShellIdentity;
use crate::runtime::config::Settings;
use crate::runtime::protocol::EnvironmentSnapshot;
mod drivers;
mod executable;
pub(crate) mod frame;
pub mod quote;
pub(crate) mod shims;
mod wrappers;
use alloc::borrow::Cow;
use anyhow::{Context as _, Result};
pub(crate) use drivers::DispatchWake;
pub(crate) use executable::ShellChoice;
use std::ffi::OsString;
use std::path::Path;
pub(crate) struct ShellStartup {
    pub(crate) args: Vec<String>,
    pub(crate) env: Vec<(OsString, OsString)>,
    pub(crate) ready_file: std::path::PathBuf,
}
impl ShellChoice {
    pub(crate) fn executable_path(
        self,
        settings: &Settings,
        environment: &EnvironmentSnapshot,
        cwd: &Path,
        identity: &ShellIdentity,
    ) -> Result<std::path::PathBuf> {
        executable::select_available_executable(
            self,
            self.executable_candidates(settings),
            environment,
            cwd,
            identity,
        )
    }
    pub(crate) fn dispatch_wakeup(self, directory: &Path) -> Result<DispatchWake> {
        drivers::dispatch_wakeup(self, directory)
    }
    pub(crate) const fn supports_separated_output(self) -> bool {
        !matches!(self, Self::NuShell)
    }
    pub(crate) const fn source_file(self) -> &'static str {
        match self {
            Self::Cmd => crate::contract::COMMAND_SCRIPT_FILE,
            Self::PowerShell
            | Self::Bash
            | Self::NuShell
            | Self::Zsh
            | Self::Bun
            | Self::Python => crate::contract::COMMAND_FILE,
        }
    }
    pub(crate) fn prepare_command(self, directory: &Path) -> Result<()> {
        drivers::prepare_command(self, directory)
    }
    pub(crate) fn keyboard_bytes(self, bytes: &[u8]) -> Cow<'_, [u8]> {
        drivers::keyboard_bytes(self, bytes)
    }
    pub(crate) fn command_source(self, source: &str) -> String {
        drivers::command_source(self, source)
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
