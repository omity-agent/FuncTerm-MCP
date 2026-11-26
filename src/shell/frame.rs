use super::{ShellChoice, ShellStartup, drivers, shims};
use crate::contract::SESSION_STATE_DIRECTORY;
use crate::runtime::protocol::EnvironmentSnapshot;
use anyhow::{Context as _, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub(crate) const ACTIVE_ROUTE_FILE: &str = "active-shell.json";
#[derive(Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub(crate) struct ShellFrame {
    pub(crate) choice: ShellChoice,
    id: u32,
}
#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct ShellRoute {
    pub(crate) frame: ShellFrame,
    pub(crate) cwd: PathBuf,
    pub(crate) continuation: Option<String>,
}
impl ShellFrame {
    pub(crate) const fn root(choice: ShellChoice) -> Self {
        Self { choice, id: 0 }
    }
    pub(crate) fn nested(choice: ShellChoice) -> Self {
        Self {
            choice,
            id: std::process::id(),
        }
    }
    pub(crate) fn from_environment() -> Result<Self> {
        let id = std::env::var(shims::FRAME_ID_ENV)?.parse()?;
        let choice = ShellChoice::from_canonical_name(&std::env::var(shims::CURRENT_SHELL_ENV)?)?;
        Ok(Self { choice, id })
    }
    pub(crate) const fn id(self) -> u32 {
        self.id
    }
    pub(crate) const fn is_root(self) -> bool {
        self.id == 0
    }
    pub(crate) fn workspace(self, tab_root: &Path) -> PathBuf {
        if self.is_root() {
            tab_root.to_path_buf()
        } else {
            tab_root.join("frames").join(self.id.to_string())
        }
    }
    pub(crate) fn state_directory(self, tab_root: &Path) -> PathBuf {
        self.workspace(tab_root).join(SESSION_STATE_DIRECTORY)
    }
    pub(crate) fn startup(
        self,
        cwd: &Path,
        tab_root: &Path,
        load_profile: bool,
        ignore_shell_style: bool,
        environment: &EnvironmentSnapshot,
    ) -> Result<ShellStartup> {
        let state_directory = self.state_directory(tab_root);
        let startup_directory = self.workspace(tab_root).join("startup");
        fs_err::create_dir_all(&state_directory)?;
        fs_err::create_dir_all(&startup_directory)?;
        let ready_file = state_directory.join("ready");
        let mut variables = environment.variables();
        for (name, value) in [
            (
                shims::CURRENT_SHELL_ENV,
                self.choice.canonical_name().into(),
            ),
            (shims::FRAME_ID_ENV, self.id.to_string().into()),
            (shims::FRAME_STATE_ENV, state_directory.into_os_string()),
            (
                shims::LOAD_PROFILE_ENV,
                u8::from(load_profile).to_string().into(),
            ),
            (
                shims::IGNORE_SHELL_STYLE_ENV,
                u8::from(ignore_shell_style).to_string().into(),
            ),
        ] {
            shims::set_environment_value(&mut variables, name, value);
        }
        if ignore_shell_style {
            shims::set_environment_value(&mut variables, shims::NO_COLOR_ENV, "1".into());
        }
        let launch_environment = EnvironmentSnapshot::from_variables(variables);
        let startup = drivers::startup(
            self.choice,
            drivers::StartupContext {
                frame_id: self.id,
                cwd,
                startup_directory: &startup_directory,
                ready_file: &ready_file,
                load_profile,
                ignore_shell_style,
                environment: &launch_environment,
            },
        )?;
        let mut env = launch_environment.variables();
        env.extend(
            startup
                .env
                .into_iter()
                .map(|(name, value)| (OsString::from(name), OsString::from(value))),
        );
        Ok(ShellStartup {
            args: startup.args,
            env,
            ready_file,
        })
    }
    pub(crate) const fn route(self, cwd: PathBuf, continuation: Option<String>) -> ShellRoute {
        ShellRoute {
            frame: self,
            cwd,
            continuation,
        }
    }
}
impl ShellRoute {
    pub(crate) fn publish(&self, tab_root: &Path) -> Result<()> {
        let path = route_file(tab_root);
        let bytes = sonic_rs::to_vec(self).context("failed to encode active shell route")?;
        crate::publication::write_replace(&path, bytes)
            .context("failed to publish active shell route")
    }
    pub(crate) fn read(tab_root: &Path) -> Result<Self> {
        let bytes =
            fs_err::read(route_file(tab_root)).context("failed to read active shell route")?;
        sonic_rs::from_slice(&bytes).context("failed to decode active shell route")
    }
    pub(crate) fn resume(tab_root: &Path, command_id: &str, cwd: &str) -> Result<()> {
        let route = Self::read(tab_root)?;
        if route.continuation.as_deref() == Some(command_id) {
            route
                .frame
                .route(PathBuf::from(cwd), None)
                .publish(tab_root)?;
        }
        Ok(())
    }
}
fn route_file(tab_root: &Path) -> PathBuf {
    tab_root
        .join(SESSION_STATE_DIRECTORY)
        .join(ACTIVE_ROUTE_FILE)
}
