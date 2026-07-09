use super::{
    process_tree,
    shell_session::{ShellSession, ShellSessionParts},
};
use crate::runtime::config::Settings;
use crate::runtime::session::records::wait_for_path;
use crate::runtime::session::terminal::{TerminalParser, lock_mutex, start_reader};
use crate::runtime::temp;
use crate::shell::{ShellChoice, shims};
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use rust_pty::{NativePtySystem, PtyChild, PtyConfig};
use std::path::Path;
use std::sync::Mutex;
use tastty_core::TerminalSize;
pub(super) struct ShellLauncher {
    settings: Settings,
    generation_root: std::path::PathBuf,
    shim_dir: std::path::PathBuf,
}
impl ShellLauncher {
    pub(super) fn new(settings: Settings) -> Result<Self> {
        let root = temp::daemon_root()?;
        temp::remove_stale_service_runtime(&root, &settings.daemon_service_name);
        let generation = runtime_generation();
        let generation_root =
            temp::generation_root(&root, &settings.daemon_service_name, &generation);
        let shim_dir = temp::shim_directory(&generation_root);
        Ok(Self {
            settings,
            generation_root,
            shim_dir,
        })
    }
    pub(super) fn launch(
        &self,
        tab_id: &str,
        starting_directory: &Path,
        starting_shell: ShellChoice,
    ) -> Result<Arc<ShellSession>> {
        let tab_root = temp::tab_root(&self.generation_root, tab_id);
        let tab_state = temp::tab_state_directory(&tab_root);
        let command_root = temp::tab_commands_directory(&tab_root);
        std::fs::create_dir_all(&tab_state).context("failed to create tab state directory")?;
        std::fs::create_dir_all(&command_root).context("failed to create command root")?;
        let mut startup = starting_shell.startup(starting_directory, &tab_root)?;
        startup.env.extend(shims::environment(
            &self.settings,
            &tab_root,
            &self.shim_dir,
            starting_shell,
        )?);
        let executable = starting_shell.executable(&self.settings)?;
        let ready_file = startup.ready_file.clone();
        let mut config_builder = PtyConfig::builder()
            .working_directory(starting_directory)
            .window_size(self.settings.terminal_cols, self.settings.terminal_rows);
        for (name, value) in startup.env {
            config_builder = config_builder.env(name, value);
        }
        let config = config_builder.build();
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("functerm-pty")
                .build()
                .context("failed to create pty runtime")?,
        );
        let (master, mut child) = runtime
            .block_on(<NativePtySystem as rust_pty::PtySystem>::spawn(
                executable,
                startup.args,
                &config,
            ))
            .context("failed to spawn shell")?;
        let process_tree = process_tree::ProcessTree::new();
        if let Err(error) = process_tree
            .attach(child.pid())
            .context("failed to guard shell process tree")
        {
            cleanup_unregistered_child(&mut child, &runtime);
            return Err(error);
        }
        let (reader, writer_half) = tokio::io::split(master);
        let boxed_writer: Box<dyn tokio::io::AsyncWrite + Send + Unpin> = Box::new(writer_half);
        let writer = Arc::new(tokio::sync::Mutex::new(boxed_writer));
        let screen = Arc::new(Mutex::new(TerminalParser::new(
            TerminalSize {
                rows: self.settings.terminal_rows,
                cols: self.settings.terminal_cols,
            },
            0,
        )));
        start_reader(Arc::clone(&screen), Arc::clone(&writer), &runtime, reader);
        let startup_timeout =
            Duration::try_from_secs_f64(self.settings.shell_startup_timeout_seconds)
                .context("shell_startup_timeout_seconds must be finite and non-negative")?;
        wait_for_shell_startup(&mut child, &ready_file, &screen, startup_timeout)?;
        let active_shell_file = tab_state.join("active-shell.txt");
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
            child: Box::new(child),
            runtime,
        })))
    }
}
fn runtime_generation() -> String {
    format!("{}-{}", std::process::id(), nanoid::nanoid!())
}
pub(super) fn wait_for_shell_startup(
    child: &mut dyn PtyChild,
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
fn cleanup_unregistered_child(child: &mut dyn PtyChild, runtime: &tokio::runtime::Runtime) {
    if let Err(error) = child.kill() {
        eprintln!("failed to kill unregistered shell child: {error}");
    }
    if let Err(error) = runtime.block_on(child.wait()) {
        eprintln!("failed to wait unregistered shell child: {error}");
    }
}
