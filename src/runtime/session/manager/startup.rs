use super::{
    process_tree,
    session::{ShellSession, ShellSessionParts},
};
use crate::runtime::config::Settings;
use crate::runtime::session::records::wait_for_path;
use crate::runtime::session::support::{
    TerminalCallbacks, TerminalParser, lock_mutex, start_reader,
};
use crate::runtime::temp;
use crate::shell::{ShellChoice, ShellStartup, shims};
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};
use std::path::Path;
use std::sync::Mutex;
pub(super) struct ShellLauncher {
    settings: Settings,
    command_root: std::path::PathBuf,
    shim_dir: std::path::PathBuf,
}
impl ShellLauncher {
    pub(super) fn new(settings: Settings) -> Result<Self> {
        let root = temp::daemon_root()?;
        temp::remove_stale_service_runtime(&root, &settings.daemon_service_name);
        let generation = runtime_generation();
        let command_root =
            temp::service_runtime_directory(&root, "commands", &settings.daemon_service_name)
                .join(&generation);
        let shim_dir =
            temp::service_runtime_directory(&root, "shell-shims", &settings.daemon_service_name)
                .join(generation);
        Ok(Self {
            settings,
            command_root,
            shim_dir,
        })
    }
    pub(super) fn launch(
        &self,
        tab_id: &str,
        starting_directory: &Path,
        starting_shell: ShellChoice,
    ) -> Result<Arc<ShellSession>> {
        let command_root = self.command_root.join(tab_id);
        std::fs::create_dir_all(&command_root).context("failed to create command root")?;
        let mut startup = starting_shell.startup(starting_directory, &command_root)?;
        startup.env.extend(shims::environment(
            &self.settings,
            &command_root,
            &self.shim_dir,
            starting_shell,
        )?);
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: self.settings.terminal_rows,
                cols: self.settings.terminal_cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("failed to open pty")?;
        let executable = starting_shell.executable(&self.settings)?;
        let mut command = CommandBuilder::new(executable);
        let ready_file = startup.ready_file.clone();
        apply_startup(&mut command, startup);
        command.cwd(starting_directory);
        let process_tree = process_tree::ProcessTree::new();
        let mut child = pair
            .slave
            .spawn_command(command)
            .context("failed to spawn shell")?;
        if let Err(error) = process_tree
            .attach(child.as_ref())
            .context("failed to guard shell process tree")
        {
            cleanup_unregistered_child(&mut child);
            return Err(error);
        }
        let reader = pair
            .master
            .try_clone_reader()
            .context("failed to clone pty reader")?;
        let writer = Arc::new(Mutex::new(
            pair.master
                .take_writer()
                .context("failed to take pty writer")?,
        ));
        let screen = Arc::new(Mutex::new(TerminalParser::new_with_callbacks(
            self.settings.terminal_rows,
            self.settings.terminal_cols,
            0,
            TerminalCallbacks::default(),
        )));
        start_reader(Arc::clone(&screen), Arc::clone(&writer), reader);
        let startup_timeout =
            Duration::try_from_secs_f64(self.settings.shell_startup_timeout_seconds)
                .context("shell_startup_timeout_seconds must be finite and non-negative")?;
        wait_for_shell_startup(&mut child, &ready_file, &screen, startup_timeout)?;
        let active_shell_file = command_root.join("active-shell.txt");
        Ok(Arc::new(ShellSession::new(ShellSessionParts {
            choice: starting_shell,
            cwd: starting_directory.to_path_buf(),
            writer,
            screen,
            busy: None,
            command_root,
            active_shell_file,
            command_start_timeout: startup_timeout,
            process_tree,
            child,
            slave: pair.slave,
        })))
    }
}
fn runtime_generation() -> String {
    format!("{}-{}", std::process::id(), nanoid::nanoid!())
}
pub(super) fn apply_startup(command: &mut CommandBuilder, startup: ShellStartup) {
    for (name, value) in startup.env {
        command.env(name, value);
    }
    command.args(startup.args);
}
pub(super) fn wait_for_shell_startup(
    child: &mut Box<dyn Child + Send + Sync>,
    ready_file: &Path,
    screen: &Arc<Mutex<TerminalParser>>,
    timeout: Duration,
) -> Result<()> {
    if let Some(status) = child.try_wait().context("failed to poll shell startup")? {
        bail!(
            "shell exited during startup with status {status}; screen: {}",
            startup_screen(screen)?
        );
    }
    if wait_for_path(ready_file, timeout)? {
        return Ok(());
    }
    if let Some(status) = child.try_wait().context("failed to poll shell startup")? {
        bail!(
            "shell exited during startup with status {status}; screen: {}",
            startup_screen(screen)?
        );
    }
    child.kill().context("failed to kill unready shell")?;
    bail!(
        "shell did not report startup readiness within {timeout:?}; screen: {}",
        startup_screen(screen)?
    );
}
fn startup_screen(screen: &Arc<Mutex<TerminalParser>>) -> Result<String> {
    let contents = lock_mutex(screen, "screen")?.screen().contents();
    Ok(contents.trim().to_owned())
}
fn cleanup_unregistered_child(child: &mut Box<dyn Child + Send + Sync>) {
    if let Err(error) = child.kill() {
        eprintln!("failed to kill unregistered shell child: {error}");
    }
    if let Err(error) = child.wait() {
        eprintln!("failed to wait unregistered shell child: {error}");
    }
}
