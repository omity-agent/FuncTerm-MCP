mod lifecycle;
#[cfg(test)]
mod tests;
use crate::config::Settings;
use crate::ipc::{EndReason, QueryResult};
use crate::session::records::{CommandRecord, command_query, create_record, wait_for_done};
use crate::session::support::{lock_mutex, start_reader};
use crate::shell::{ShellChoice, ShellStartup};
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use lifecycle::{release_shell, reserve_shell};
use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::thread;
use uuid::Uuid;
pub(crate) struct Manager {
    settings: Settings,
    root: std::path::PathBuf,
    shells: Mutex<HashMap<Uuid, Arc<ShellSession>>>,
    commands: Mutex<HashMap<Uuid, CommandRecord>>,
}
pub(super) struct ShellSession {
    choice: ShellChoice,
    writer: Mutex<Box<dyn Write + Send>>,
    screen: Arc<Mutex<vt100::Parser>>,
    busy: Mutex<Option<Uuid>>,
    command_root: std::path::PathBuf,
    child: Mutex<Box<dyn Child + Send + Sync>>,
}
impl Manager {
    pub(crate) fn new(settings: Settings) -> Result<Self> {
        let root = std::env::temp_dir().join("agent").join("shell-mcp-pty");
        fs::create_dir_all(&root).context("failed to create daemon temp root")?;
        Ok(Self {
            settings,
            root,
            shells: Mutex::new(HashMap::new()),
            commands: Mutex::new(HashMap::new()),
        })
    }
    pub(crate) fn new_shell(&self, cwd: &Path, shell: ShellChoice) -> Result<Uuid> {
        if !cwd.is_dir() {
            bail!(
                "cwd does not exist or is not a directory: {}",
                cwd.display()
            );
        }
        let shell_id = Uuid::new_v4();
        let command_root = self.root.join("commands").join(shell_id.to_string());
        fs::create_dir_all(&command_root).context("failed to create command root")?;
        let startup = shell.startup(cwd, &command_root)?;
        let pty_system = native_pty_system();
        let size = PtySize {
            rows: self.settings.terminal_rows,
            cols: self.settings.terminal_cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system.openpty(size).context("failed to open pty")?;
        let mut command = CommandBuilder::new(shell.executable(&self.settings));
        apply_startup(&mut command, startup);
        command.cwd(cwd);
        let child = pair
            .slave
            .spawn_command(command)
            .context("failed to spawn shell")?;
        let reader = pair
            .master
            .try_clone_reader()
            .context("failed to clone pty reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("failed to take pty writer")?;
        let screen = Arc::new(Mutex::new(vt100::Parser::new(
            self.settings.terminal_rows,
            self.settings.terminal_cols,
            0,
        )));
        start_reader(Arc::clone(&screen), reader);
        let session = Arc::new(ShellSession {
            choice: shell,
            writer: Mutex::new(writer),
            screen,
            busy: Mutex::new(None),
            command_root,
            child: Mutex::new(child),
        });
        lock_mutex(&self.shells, "shell")?.insert(shell_id, session);
        Ok(shell_id)
    }
    pub(crate) fn write_keyboard(&self, shell_id: Uuid, bytes: &[u8]) -> Result<()> {
        let shell = self.shell(shell_id)?;
        let mut writer = lock_mutex(&shell.writer, "writer")?;
        writer.write_all(bytes).context("failed to write to pty")?;
        writer.flush().context("failed to flush pty writer")
    }
    pub(crate) fn send_command(
        self: &Arc<Self>,
        shell_id: Uuid,
        command: &str,
        wait_ms: u64,
    ) -> Result<(Uuid, EndReason)> {
        let shell = self.shell(shell_id)?;
        let command_id = Uuid::new_v4();
        reserve_shell(&shell, command_id)?;
        let record = create_record(&shell.command_root, command_id)?;
        lock_mutex(&self.commands, "command")?.insert(command_id, record.clone());
        if let Err(error) = Self::write_invocation(&shell, command_id, command, &record) {
            release_shell(&shell, command_id)?;
            lock_mutex(&self.commands, "command")?.remove(&command_id);
            return Err(error);
        }
        self.start_monitor(command_id, shell, record.clone());
        let ended = wait_for_done(&record.done, Duration::from_millis(wait_ms));
        let reason = if ended {
            EndReason::CommandEnded
        } else {
            EndReason::WaitTimeout
        };
        Ok((command_id, reason))
    }
    pub(crate) fn query(&self, id: Uuid) -> Result<QueryResult> {
        if let Some(shell) = self.find_shell(id)? {
            let screen = lock_mutex(&shell.screen, "screen")?.screen().contents();
            return Ok(QueryResult::Shell { screen });
        }
        if let Some(record) = self.find_command(id)? {
            return command_query(&record);
        }
        bail!("unknown UUID {id}")
    }
    fn find_shell(&self, id: Uuid) -> Result<Option<Arc<ShellSession>>> {
        Ok(lock_mutex(&self.shells, "shell")?.get(&id).cloned())
    }
    fn find_command(&self, id: Uuid) -> Result<Option<CommandRecord>> {
        Ok(lock_mutex(&self.commands, "command")?.get(&id).cloned())
    }
    fn shell(&self, shell_id: Uuid) -> Result<Arc<ShellSession>> {
        self.find_shell(shell_id)?
            .with_context(|| format!("unknown shell UUID {shell_id}"))
    }
    fn write_invocation(
        shell: &ShellSession,
        command_id: Uuid,
        command: &str,
        record: &CommandRecord,
    ) -> Result<()> {
        let directory = record
            .stdout
            .parent()
            .context("missing command directory")?;
        let line = shell.choice.invocation(command_id, command, directory);
        let mut writer = lock_mutex(&shell.writer, "writer")?;
        writer
            .write_all(line.as_bytes())
            .context("failed to write command invocation")?;
        writer.flush().context("failed to flush command invocation")
    }
    fn start_monitor(
        self: &Arc<Self>,
        command_id: Uuid,
        shell: Arc<ShellSession>,
        record: CommandRecord,
    ) {
        let manager = Arc::clone(self);
        thread::spawn(move || {
            while !record.done.exists() {
                thread::sleep(Duration::from_millis(100));
            }
            if let Ok(mut busy) = shell.busy.lock() {
                *busy = None;
            }
            if let Ok(mut commands) = manager.commands.lock() {
                commands.entry(command_id).or_insert(record);
            }
        });
    }
}
fn apply_startup(command: &mut CommandBuilder, startup: ShellStartup) {
    command.args(startup.args);
}
