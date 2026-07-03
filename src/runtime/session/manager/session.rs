use super::process_tree;
use crate::runtime::session::records::{CommandRecord, read_done};
use crate::runtime::session::support::lock_mutex;
use crate::shell::{ShellChoice, shims};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use base64_turbo::STANDARD;
use core::time::Duration;
use portable_pty::{Child, SlavePty};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
pub(super) struct ShellSession {
    choice: Mutex<ShellChoice>,
    cwd: Mutex<PathBuf>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    screen: Arc<Mutex<vt100::Parser>>,
    last_command: Mutex<Option<String>>,
    busy: Mutex<Option<String>>,
    command_root: PathBuf,
    active_shell_file: PathBuf,
    command_start_timeout: Duration,
    process_tree: process_tree::ProcessTree,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    _slave: Mutex<Box<dyn SlavePty + Send>>,
}
#[derive(Debug)]
pub(super) enum KeyboardWriteFailure {
    IdlePrompt,
    Write(anyhow::Error),
}
pub(super) struct ShellSessionParts {
    pub(super) choice: ShellChoice,
    pub(super) cwd: PathBuf,
    pub(super) writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub(super) screen: Arc<Mutex<vt100::Parser>>,
    pub(super) last_command: Option<String>,
    pub(super) busy: Option<String>,
    pub(super) command_root: PathBuf,
    pub(super) active_shell_file: PathBuf,
    pub(super) command_start_timeout: Duration,
    pub(super) process_tree: process_tree::ProcessTree,
    pub(super) child: Box<dyn Child + Send + Sync>,
    pub(super) slave: Box<dyn SlavePty + Send>,
}
impl ShellSession {
    pub(super) fn new(parts: ShellSessionParts) -> Self {
        Self {
            choice: Mutex::new(parts.choice),
            cwd: Mutex::new(parts.cwd),
            writer: parts.writer,
            screen: parts.screen,
            last_command: Mutex::new(parts.last_command),
            busy: Mutex::new(parts.busy),
            command_root: parts.command_root,
            active_shell_file: parts.active_shell_file,
            command_start_timeout: parts.command_start_timeout,
            process_tree: parts.process_tree,
            child: Mutex::new(parts.child),
            _slave: Mutex::new(parts.slave),
        }
    }
    pub(super) fn cwd(&self) -> Result<PathBuf> {
        Ok(lock_mutex(&self.cwd, "cwd")?.clone())
    }
    pub(super) fn command_root(&self) -> &Path {
        &self.command_root
    }
    pub(super) fn set_cwd(&self, cwd: PathBuf) -> Result<()> {
        *lock_mutex(&self.cwd, "cwd")? = cwd;
        Ok(())
    }
    pub(super) fn set_last_command(&self, command: String) -> Result<()> {
        *lock_mutex(&self.last_command, "last command")? = Some(command);
        Ok(())
    }
    pub(super) fn last_command(&self) -> Result<Option<String>> {
        Ok(lock_mutex(&self.last_command, "last command")?.clone())
    }
    pub(super) fn screen_contents(&self) -> Result<String> {
        Ok(lock_mutex(&self.screen, "screen")?.screen().contents())
    }
    pub(super) fn current_choice(&self) -> Result<ShellChoice> {
        Ok(*lock_mutex(&self.choice, "choice")?)
    }
    pub(super) fn refresh_choice(&self) -> Result<()> {
        if let Some(choice) = shims::read_active_shell(&self.active_shell_file)? {
            *lock_mutex(&self.choice, "choice")? = choice;
        }
        Ok(())
    }
    pub(super) fn is_alive(&self) -> Result<bool> {
        let status = lock_mutex(&self.child, "child")?
            .try_wait()
            .context("failed to poll shell child")?;
        Ok(status.is_none())
    }
    pub(super) fn write_keyboard_for_running_command(
        &self,
        bytes: &[u8],
    ) -> Result<(), KeyboardWriteFailure> {
        let busy = lock_mutex(&self.busy, "busy").map_err(KeyboardWriteFailure::Write)?;
        if busy.is_none() {
            return Err(KeyboardWriteFailure::IdlePrompt);
        }
        let write_result = self.write_keyboard(bytes);
        drop(busy);
        write_result.map_err(KeyboardWriteFailure::Write)
    }
    fn write_keyboard(&self, bytes: &[u8]) -> Result<()> {
        let choice = self.current_choice()?;
        let keyboard_bytes = choice.keyboard_bytes(bytes);
        let mut writer = lock_mutex(&self.writer, "writer")?;
        writer
            .write_all(&keyboard_bytes)
            .context("failed to write to pty")?;
        writer.flush().context("failed to flush pty writer")
    }
    pub(super) fn write_invocation(
        &self,
        command_id: &str,
        command: &str,
        record: &CommandRecord,
    ) -> Result<()> {
        let payload = STANDARD.encode(command.as_bytes());
        std::fs::write(&record.payload, payload).context("failed to write command payload")?;
        let directory = record
            .stdout
            .parent()
            .context("missing command directory")?;
        let line = self
            .current_choice()?
            .invocation(command_id, directory, &record.initial_cwd)?;
        let mut writer = lock_mutex(&self.writer, "writer")?;
        writer
            .write_all(line.as_bytes())
            .context("failed to write command invocation")?;
        writer.flush().context("failed to flush command invocation")
    }
    pub(super) fn update_cwd_from_done(&self, record: &CommandRecord) -> Result<()> {
        if let Some(done) = read_done(&record.done)? {
            self.set_cwd(PathBuf::from(done.cwd))?;
        }
        Ok(())
    }
    pub(super) fn wait_for_command_start(&self, record: &CommandRecord) -> Result<()> {
        if crate::runtime::session::records::wait_for_start_or_done(
            record,
            self.command_start_timeout,
        )? {
            return Ok(());
        }
        anyhow::bail!(
            "shell did not start command within {:?}",
            self.command_start_timeout
        );
    }
    pub(super) fn reserve(&self, command_id: &str) -> Result<()> {
        let mut busy = lock_mutex(&self.busy, "busy")?;
        if let Some(existing_id) = busy.as_deref() {
            anyhow::bail!("shell is busy with command {existing_id}");
        }
        *busy = Some(command_id.to_owned());
        drop(busy);
        Ok(())
    }
    pub(super) fn release(&self, command_id: &str) -> Result<()> {
        let mut busy = lock_mutex(&self.busy, "busy")?;
        if busy.as_deref() == Some(command_id) {
            *busy = None;
        }
        drop(busy);
        Ok(())
    }
    pub(super) fn busy_command_id(&self) -> Result<Option<String>> {
        Ok(lock_mutex(&self.busy, "busy")?.clone())
    }
}
impl Drop for ShellSession {
    fn drop(&mut self) {
        if let Err(error) = self.process_tree.terminate() {
            eprintln!("failed to terminate shell process tree during cleanup: {error}");
        }
        if let Ok(mut child) = self.child.lock() {
            match child.try_wait() {
                Ok(Some(_)) => {}
                Ok(None) => {
                    if let Err(error) = child.kill() {
                        eprintln!("failed to kill shell child during cleanup: {error}");
                    }
                    if let Err(error) = child.wait() {
                        eprintln!("failed to wait shell child during cleanup: {error}");
                    }
                }
                Err(error) => eprintln!("failed to poll shell child during cleanup: {error}"),
            }
        }
    }
}
