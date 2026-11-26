#[cfg(windows)]
mod access_token;
#[cfg(windows)]
pub(crate) mod desktop_host;
pub(crate) mod query_process;
#[cfg(windows)]
mod relay_process;
#[cfg(windows)]
pub(crate) mod stream_host;
#[cfg(windows)]
pub(super) mod tree_guard;
#[cfg(unix)]
mod unix_account;
#[cfg(unix)]
mod workspace_acl;
#[cfg(windows)]
pub(crate) use access_token::ShellIdentity;
#[cfg(windows)]
pub(crate) use access_token::user_sid;
#[cfg(unix)]
use anyhow::{Context as _, Result};
use clap::ValueEnum;
#[cfg(unix)]
use portable_pty::CommandBuilder;
use portable_pty::{Child, ChildKiller, MasterPty, SlavePty};
#[cfg(windows)]
pub(crate) use relay_process::spawn as spawn_normal_host;
#[cfg(unix)]
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
#[cfg(unix)]
pub(crate) use unix_account::ShellIdentity;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(crate) use workspace_acl::inherit_publication_acl;
#[derive(Clone, Copy, Debug, ValueEnum, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PrivilegeMode {
    Normal,
    Elevated,
}
impl PrivilegeMode {
    pub(crate) const fn elevated(self) -> bool {
        matches!(self, Self::Elevated)
    }
}
pub(crate) struct PtyProcess {
    pub(crate) child: Box<dyn Child + Send + Sync>,
    pub(crate) reader: Box<dyn Read + Send>,
    pub(crate) writer: Box<dyn Write + Send>,
    pub(crate) slave: Option<Box<dyn SlavePty + Send>>,
    pub(crate) master: Option<Box<dyn MasterPty + Send>>,
    pub(crate) tree_killer: Option<Box<dyn ChildKiller + Send + Sync>>,
}
#[cfg(unix)]
impl ShellIdentity {
    pub(crate) fn configure(&self, command: &mut CommandBuilder) -> Result<()> {
        let arguments = self.command_line(command.get_argv())?;
        *command.get_argv_mut() = arguments;
        Ok(())
    }
    fn command_line(
        &self,
        arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> Result<Vec<OsString>> {
        let mut argv = Vec::new();
        if let Some(options) = self.helper_arguments() {
            let executable = std::env::current_exe().context("failed to resolve shell launcher")?;
            argv.extend([executable.into_os_string(), "internal-run-shell".into()]);
            argv.extend(options);
            argv.push("--".into());
        }
        argv.extend(arguments.into_iter().map(|arg| arg.as_ref().to_owned()));
        Ok(argv)
    }
}
#[cfg(unix)]
pub(crate) fn run_shell(uid: u32, arguments: &[OsString]) -> Result<()> {
    unix_account::run(uid, arguments)
}
