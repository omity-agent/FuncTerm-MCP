use super::process;
use crate::runtime::session::keyboard;
use crate::runtime::session::records::{CommandRecord, read_done};
use crate::runtime::session::terminal::{TerminalParser, TerminalWriter, lock_mutex, screen_title};
use crate::shell::{ShellChoice, shims};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
use rmux_pty::PtyChild;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread::JoinHandle;
pub(super) struct ShellSession {
    choice: Mutex<ShellChoice>,
    cwd: Mutex<PathBuf>,
    writer: TerminalWriter,
    screen: Arc<Mutex<TerminalParser>>,
    busy: Mutex<Option<String>>,
    command_root: PathBuf,
    active_shell_file: PathBuf,
    command_start_timeout: Duration,
    child: Mutex<PtyChild>,
    reader: Option<JoinHandle<()>>,
}
#[derive(Debug)]
pub(super) enum KeyboardWriteFailure {
    IdlePrompt,
    Write(anyhow::Error),
}
pub(super) struct ShellSessionParts {
    pub(super) choice: ShellChoice,
    pub(super) cwd: PathBuf,
    pub(super) writer: TerminalWriter,
    pub(super) screen: Arc<Mutex<TerminalParser>>,
    pub(super) busy: Option<String>,
    pub(super) command_root: PathBuf,
    pub(super) active_shell_file: PathBuf,
    pub(super) command_start_timeout: Duration,
    pub(super) child: PtyChild,
    pub(super) reader: Option<JoinHandle<()>>,
}
impl ShellSession {
    pub(super) fn new(parts: ShellSessionParts) -> Self {
        Self {
            choice: Mutex::new(parts.choice),
            cwd: Mutex::new(parts.cwd),
            writer: parts.writer,
            screen: parts.screen,
            busy: Mutex::new(parts.busy),
            command_root: parts.command_root,
            active_shell_file: parts.active_shell_file,
            command_start_timeout: parts.command_start_timeout,
            child: Mutex::new(parts.child),
            reader: parts.reader,
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
    pub(super) fn screen_contents(&self) -> Result<String> {
        Ok(lock_mutex(&self.screen, "screen")?.screen().contents())
    }
    pub(super) fn screen_title(&self) -> Result<String> {
        let parser = lock_mutex(&self.screen, "screen")?;
        Ok(screen_title(&parser))
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
        let mut child = lock_mutex(&self.child, "child")?;
        let alive = process::is_alive(&mut child)?;
        drop(child);
        Ok(alive)
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
        let physical_bytes = keyboard::physical_bytes(keyboard_bytes.as_ref());
        self.write_bytes(physical_bytes.as_ref(), "failed to write to pty")
    }
    pub(super) fn write_invocation(
        &self,
        command_id: &str,
        command: &str,
        record: &CommandRecord,
    ) -> Result<()> {
        std::fs::write(&record.command, command).context("failed to write command")?;
        std::fs::write(&record.script, command).context("failed to write command script")?;
        let line = self.current_choice()?.invocation(
            command_id,
            &record.directory,
            &record.initial_cwd,
        )?;
        self.write_bytes(line.as_bytes(), "failed to write command invocation")
    }
    fn write_bytes(&self, bytes: &[u8], write_context: &'static str) -> Result<()> {
        lock_mutex(&self.writer, "pty writer")?
            .write_all(bytes)
            .context(write_context)
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
        let child = match self.child.get_mut() {
            Ok(child) => child,
            Err(error) => {
                eprintln!("child mutex poisoned during shell cleanup");
                error.into_inner()
            }
        };
        process::cleanup(child, "shell child during cleanup");
        if let Some(reader) = self.reader.take() {
            process::join_reader(reader, "pty reader thread");
        }
    }
}
