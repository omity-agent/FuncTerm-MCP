mod commands;
mod lifecycle;
mod process_tree;
mod startup;
mod state;
#[cfg(test)]
mod tests;
use crate::runtime::config::Settings;
use crate::runtime::protocol::ViewResult;
use crate::runtime::session::records::{CommandRecord, command_query, wait_for_done};
use crate::runtime::session::support::{lock_mutex, start_reader};
use crate::runtime::temp;
use crate::shell::{ShellChoice, shims};
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use portable_pty::{Child, CommandBuilder, PtySize, SlavePty, native_pty_system};
use startup::{apply_startup, wait_for_shell_startup};
use state::path_text;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::thread;
pub(crate) struct Manager {
    settings: Settings,
    root: std::path::PathBuf,
    shells: Mutex<HashMap<String, Arc<ShellSession>>>,
    commands: Mutex<HashMap<String, CommandRecord>>,
}
pub(super) struct ShellSession {
    choice: Mutex<ShellChoice>,
    cwd: Mutex<std::path::PathBuf>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    screen: Arc<Mutex<vt100::Parser>>,
    busy: Mutex<Option<String>>,
    command_root: std::path::PathBuf,
    active_shell_file: std::path::PathBuf,
    process_tree: process_tree::ProcessTree,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    _slave: Mutex<Box<dyn SlavePty + Send>>,
}
impl Manager {
    pub(crate) fn new(settings: Settings) -> Result<Self> {
        let root = temp::daemon_root()?;
        Ok(Self {
            settings,
            root,
            shells: Mutex::new(HashMap::new()),
            commands: Mutex::new(HashMap::new()),
        })
    }
    pub(crate) fn new_tab(
        &self,
        starting_directory: &Path,
        starting_shell: ShellChoice,
    ) -> Result<String> {
        if !starting_directory.is_dir() {
            bail!(
                "starting_directory does not exist or is not a directory: {}",
                starting_directory.display()
            );
        }
        let tab_id = self.next_tab_id()?;
        let command_root = self.root.join("commands").join(&tab_id);
        std::fs::create_dir_all(&command_root).context("failed to create command root")?;
        let mut startup = starting_shell.startup(starting_directory, &command_root)?;
        startup.env.extend(shims::environment(
            &self.settings,
            &command_root,
            starting_shell,
        )?);
        let pty_system = native_pty_system();
        let size = PtySize {
            rows: self.settings.terminal_rows,
            cols: self.settings.terminal_cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system.openpty(size).context("failed to open pty")?;
        let executable = starting_shell.executable(&self.settings)?;
        let mut command = CommandBuilder::new(executable);
        let ready_file = startup.ready_file.clone();
        apply_startup(&mut command, startup);
        command.cwd(starting_directory);
        let process_tree =
            process_tree::ProcessTree::new().context("failed to create shell cleanup guard")?;
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
        let screen = Arc::new(Mutex::new(vt100::Parser::new(
            self.settings.terminal_rows,
            self.settings.terminal_cols,
            0,
        )));
        start_reader(Arc::clone(&screen), Arc::clone(&writer), reader);
        wait_for_shell_startup(&mut child, &ready_file, &screen)?;
        let active_shell_file = command_root.join("active-shell.txt");
        shims::write_active_shell(&active_shell_file, starting_shell)?;
        let session = Arc::new(ShellSession {
            choice: Mutex::new(starting_shell),
            cwd: Mutex::new(starting_directory.to_path_buf()),
            writer,
            screen,
            busy: Mutex::new(None),
            command_root,
            active_shell_file,
            process_tree,
            child: Mutex::new(child),
            _slave: Mutex::new(pair.slave),
        });
        lock_mutex(&self.shells, "shell")?.insert(tab_id.clone(), session);
        Ok(tab_id)
    }
    pub(crate) fn view(&self, id: &str, waiting: core::time::Duration) -> Result<ViewResult> {
        if let Some(shell) = self.find_shell(id)? {
            self.wait_for_shell_view(&shell, waiting)?;
            return Self::tab_view(&shell);
        }
        if let Some(record) = self.find_command(id)? {
            self.wait_for_command_view(&record, waiting)?;
            return self.command_view(&record);
        }
        bail!("unknown id {id}")
    }
    fn tab_view(shell: &Arc<ShellSession>) -> Result<ViewResult> {
        let alive = Self::shell_alive(shell)?;
        Self::refresh_shell_choice(shell)?;
        let screen = lock_mutex(&shell.screen, "screen")?.screen().contents();
        let cwd = path_text(&Self::shell_cwd(shell)?)?;
        Ok(ViewResult::Tab { alive, cwd, screen })
    }
    fn command_view(&self, record: &CommandRecord) -> Result<ViewResult> {
        let fallback_cwd = self.command_fallback_cwd(record)?;
        command_query(record, &fallback_cwd)
    }
    fn wait_for_shell_view(
        &self,
        shell: &Arc<ShellSession>,
        waiting: core::time::Duration,
    ) -> Result<()> {
        let busy_command_id = lock_mutex(&shell.busy, "busy")?.clone();
        let Some(command_id) = busy_command_id else {
            thread::sleep(waiting);
            return Ok(());
        };
        let record = self
            .find_command(&command_id)?
            .with_context(|| format!("busy shell command is missing: {command_id}"))?;
        if wait_for_done(&record.done, waiting)? {
            Self::update_shell_cwd(shell, &record)?;
        }
        Ok(())
    }
    fn wait_for_command_view(
        &self,
        record: &CommandRecord,
        waiting: core::time::Duration,
    ) -> Result<()> {
        if wait_for_done(&record.done, waiting)?
            && let Some(shell) = self.find_shell(&record.tab_id)?
        {
            Self::update_shell_cwd(&shell, record)?;
        }
        Ok(())
    }
    fn find_shell(&self, id: &str) -> Result<Option<Arc<ShellSession>>> {
        Ok(lock_mutex(&self.shells, "shell")?.get(id).cloned())
    }
    fn find_command(&self, id: &str) -> Result<Option<CommandRecord>> {
        Ok(lock_mutex(&self.commands, "command")?.get(id).cloned())
    }
    fn shell_alive(shell: &ShellSession) -> Result<bool> {
        let status = lock_mutex(&shell.child, "child")?
            .try_wait()
            .context("failed to poll shell child")?;
        Ok(status.is_none())
    }
}
fn cleanup_unregistered_child(child: &mut Box<dyn Child + Send + Sync>) {
    if let Err(error) = child.kill() {
        eprintln!("failed to kill unregistered shell child: {error}");
    }
    if let Err(error) = child.wait() {
        eprintln!("failed to wait unregistered shell child: {error}");
    }
}
