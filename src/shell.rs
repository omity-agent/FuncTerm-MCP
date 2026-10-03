use crate::runtime::config::Settings;
use crate::runtime::protocol::EnvironmentSnapshot;
mod drivers;
mod executable;
pub mod quote;
pub(crate) mod shims;
mod wrappers;
use alloc::borrow::Cow;
use anyhow::{Context as _, Result};
pub(crate) use executable::ShellChoice;
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
    pub(crate) fn startup(
        self,
        cwd: &Path,
        session_root: &Path,
        load_profile: bool,
        environment: &EnvironmentSnapshot,
    ) -> Result<ShellStartup> {
        let state_directory = session_root.join("state");
        let startup_directory = session_root.join("startup");
        fs_err::create_dir_all(&state_directory)?;
        fs_err::create_dir_all(&startup_directory)?;
        let ready_file = state_directory.join("ready");
        let mut variables = environment.variables();
        variables.retain(|pair| {
            !crate::runtime::protocol::environment_name_equals(&pair.0, shims::LOAD_PROFILE_ENV)
        });
        variables.push((
            shims::LOAD_PROFILE_ENV.into(),
            if load_profile { "1" } else { "0" }.into(),
        ));
        let launch_environment = EnvironmentSnapshot::from_variables(variables);
        let startup = drivers::startup(
            self,
            drivers::StartupContext {
                cwd,
                startup_directory: &startup_directory,
                ready_file: &ready_file,
                load_profile,
                environment: &launch_environment,
            },
        )?;
        let args = startup.args;
        let mut env = launch_environment.variables();
        env.extend(
            startup
                .env
                .into_iter()
                .map(|(name, value)| (OsString::from(name), OsString::from(value))),
        );
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
