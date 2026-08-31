mod cleanup;
mod input;
use super::{command::ManagedCommand, process, process_tree};
use crate::runtime::session::records::{CommandRecord, read_done};
use crate::runtime::session::terminal::{CommandTitle, Terminal};
use crate::shell::{ShellChoice, shims};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
use parking_lot::Mutex;
use portable_pty::{Child, SlavePty};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
pub(super) struct ShellSession {
    choice: Mutex<ShellChoice>,
    cwd: Mutex<PathBuf>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    screen: Arc<Terminal>,
    busy: Mutex<Option<Arc<ManagedCommand>>>,
    command_root: PathBuf,
    dispatch_file: PathBuf,
    active_shell_file: PathBuf,
    command_start_timeout: Duration,
    process_tree: process_tree::ProcessTree,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    slave: Mutex<Option<Box<dyn SlavePty + Send>>>,
    reader: Option<JoinHandle<()>>,
}
pub(in crate::engine::runtime::session::manager) use input::KeyboardWriteFailure;
pub(super) struct ShellSessionParts {
    pub(super) choice: ShellChoice,
    pub(super) cwd: PathBuf,
    pub(super) writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub(super) screen: Arc<Terminal>,
    pub(super) busy: Option<Arc<ManagedCommand>>,
    pub(super) command_root: PathBuf,
    pub(super) dispatch_file: PathBuf,
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
            dispatch_file: parts.dispatch_file,
            active_shell_file: parts.active_shell_file,
            command_start_timeout: parts.command_start_timeout,
            process_tree: parts.process_tree,
            child: Mutex::new(parts.child),
            slave: Mutex::new(parts.slave),
            reader: parts.reader,
        }
    }
    pub(super) fn cwd(&self) -> PathBuf {
        self.cwd.lock().clone()
    }
    pub(super) fn command_root(&self) -> &Path {
        &self.command_root
    }
    pub(super) fn set_cwd(&self, cwd: PathBuf) {
        *self.cwd.lock() = cwd;
    }
    pub(super) fn screen_contents(&self) -> String {
        self.screen.contents()
    }
    pub(super) fn model_title(&self) -> String {
        self.screen.model_title()
    }
    pub(super) fn capture_title(&self, command_id: &str) -> Result<Arc<CommandTitle>> {
        self.screen.capture_title(command_id)
    }
    pub(super) fn current_choice(&self) -> ShellChoice {
        *self.choice.lock()
    }
    pub(super) fn refresh_choice(&self) -> Result<()> {
        if let Some(choice) = shims::read_active_shell(&self.active_shell_file)? {
            *self.choice.lock() = choice;
        }
        Ok(())
    }
    pub(super) fn is_alive(&self) -> Result<bool> {
        process::is_alive(self.child.lock().as_mut())
    }
    pub(super) fn write_invocation(
        &self,
        command_id: &str,
        command: &str,
        record: &CommandRecord,
    ) -> Result<()> {
        fs_err::write(&record.command, command)?;
        let choice = self.current_choice();
        fs_err::write(record.script_for(choice), choice.command_script(command))?;
        crate::file_publish::write_replace(&self.dispatch_file, command_id)
            .context("failed to publish command dispatch")?;
        let Some(invocation) = choice.invocation()? else {
            return Ok(());
        };
        let invocation_bytes = invocation.into_bytes();
        let mut writer = self.writer.lock();
        writer
            .write_all(&invocation_bytes)
            .context("failed to write command invocation")?;
        writer.flush().context("failed to flush command invocation")
    }
    pub(super) fn update_cwd_from_done(&self, record: &CommandRecord) -> Result<()> {
        if let Some(done) = read_done(&record.done)? {
            self.set_cwd(PathBuf::from(done.cwd));
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
}
