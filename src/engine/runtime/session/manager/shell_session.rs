mod cleanup;
use super::{process, process_tree};
use crate::runtime::protocol::KeyboardInput;
use crate::runtime::session::keyboard;
use crate::runtime::session::records::{CommandRecord, read_done};
use crate::runtime::session::terminal::{CommandTitle, Terminal, lock_mutex};
use crate::shell::{ShellChoice, shims};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
use portable_pty::{Child, SlavePty};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread::JoinHandle;
pub(super) struct ShellSession {
    choice: Mutex<ShellChoice>,
    cwd: Mutex<PathBuf>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    screen: Arc<Terminal>,
    busy: Mutex<Option<String>>,
    command_root: PathBuf,
    active_shell_file: PathBuf,
    command_start_timeout: Duration,
    process_tree: process_tree::ProcessTree,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    slave: Mutex<Option<Box<dyn SlavePty + Send>>>,
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
    pub(super) writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub(super) screen: Arc<Terminal>,
    pub(super) busy: Option<String>,
    pub(super) command_root: PathBuf,
    pub(super) active_shell_file: PathBuf,
    pub(super) command_start_timeout: Duration,
    pub(super) process_tree: process_tree::ProcessTree,
    pub(super) child: Box<dyn Child + Send + Sync>,
    pub(super) slave: Option<Box<dyn SlavePty + Send>>,
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
            process_tree: parts.process_tree,
            child: Mutex::new(parts.child),
            slave: Mutex::new(parts.slave),
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
        self.screen.contents()
    }
    pub(super) fn model_title(&self) -> String {
        self.screen.model_title()
    }
    pub(super) fn capture_title(&self, command_id: &str) -> Result<Arc<CommandTitle>> {
        self.screen.capture_title(command_id)
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
        let alive = process::is_alive(child.as_mut())?;
        drop(child);
        Ok(alive)
    }
    pub(super) fn write_keyboard_for_running_command(
        &self,
        input: KeyboardInput,
        waiting: Duration,
    ) -> Result<(), KeyboardWriteFailure> {
        let busy = lock_mutex(&self.busy, "busy").map_err(KeyboardWriteFailure::Write)?;
        if busy.is_none() {
            return Err(KeyboardWriteFailure::IdlePrompt);
        }
        let revision = self
            .screen
            .output_revision()
            .map_err(KeyboardWriteFailure::Write)?;
        let write_result = self.write_keyboard(input);
        drop(busy);
        write_result.map_err(KeyboardWriteFailure::Write)?;
        self.screen
            .wait_for_output(revision, waiting)
            .map_err(KeyboardWriteFailure::Write)
    }
    fn write_keyboard(&self, input: KeyboardInput) -> Result<()> {
        let shell_bytes = match input {
            KeyboardInput::Text(text) => self
                .current_choice()?
                .keyboard_bytes(text.as_bytes())
                .into_owned(),
            KeyboardInput::Bytes(bytes) => bytes,
        };
        let physical_bytes = keyboard::user_bytes(&shell_bytes);
        let mut writer = lock_mutex(&self.writer, "writer")?;
        writer
            .write_all(physical_bytes.as_ref())
            .context("failed to write to pty")?;
        writer.flush().context("failed to flush pty writer")
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
