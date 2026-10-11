use super::super::process;
use crate::engine::privileges::{PtyProcess, ShellIdentity};
use crate::shell::ShellStartup;
use anyhow::{Context as _, Result};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::path::Path;
pub(super) fn launch(
    executable: &str,
    startup: ShellStartup,
    starting_directory: &Path,
    identity: &ShellIdentity,
    size: PtySize,
) -> Result<PtyProcess> {
    #[cfg(windows)]
    if identity.needs_normal_host() {
        return crate::engine::privileges::spawn_normal_host(
            executable,
            startup,
            starting_directory,
            size,
        );
    }
    let mut command = CommandBuilder::new(executable);
    command.env_clear();
    apply_startup(&mut command, startup);
    command.cwd(starting_directory);
    #[cfg(unix)]
    identity.configure(&mut command)?;
    let pair = native_pty_system()
        .openpty(size)
        .context("failed to open pty")?;
    let mut child = pair
        .slave
        .spawn_command(command)
        .context("failed to spawn shell")?;
    let reader = pair
        .master
        .try_clone_reader()
        .inspect_err(|_error| {
            process::cleanup(child.as_mut(), "unregistered shell child");
        })
        .context("failed to clone pty reader")?;
    let writer = pair
        .master
        .take_writer()
        .inspect_err(|_error| {
            process::cleanup(child.as_mut(), "unregistered shell child");
        })
        .context("failed to take pty writer")?;
    #[cfg(windows)]
    let slave = Some(pair.slave);
    #[cfg(unix)]
    let slave = {
        drop(pair.slave);
        None
    };
    Ok(PtyProcess {
        child,
        reader,
        writer,
        slave,
        master: Some(pair.master),
    })
}
fn apply_startup(command: &mut CommandBuilder, startup: ShellStartup) {
    for (name, value) in startup.env {
        command.env(name, value);
    }
    command.args(startup.args);
}
