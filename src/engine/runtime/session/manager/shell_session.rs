mod cleanup;
#[cfg(windows)]
mod containment;
mod input;
pub(super) mod process;
use super::command::ManagedCommand;
use crate::runtime::session::observation::PathWatch;
use crate::runtime::session::records::CommandRecord;
use crate::runtime::session::terminal::{CommandCapture, Terminal};
use crate::shell::{ShellChoice, shims};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
use parking_lot::Mutex;
use portable_pty::{Child, SlavePty};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
#[derive(bon::Builder)]
pub(super) struct ShellSession {
    #[builder(with = |value: ShellChoice| Mutex::new(value))]
    choice: Mutex<ShellChoice>,
    #[builder(with = |value: PathBuf| Mutex::new(value))]
    cwd: Mutex<PathBuf>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    screen: Arc<Terminal>,
    #[builder(skip)]
    busy: Mutex<Option<Arc<ManagedCommand>>>,
    #[builder(skip)]
    terminated: Mutex<bool>,
    command_root: PathBuf,
    command_watch: Arc<PathWatch>,
    dispatch_file: PathBuf,
    active_shell_file: PathBuf,
    command_start_timeout: Duration,
    process_tree: process::ProcessTree,
    #[builder(with = |value: Box<dyn Child + Send + Sync>| Mutex::new(value))]
    child: Mutex<Box<dyn Child + Send + Sync>>,
    #[builder(with = |value: Option<Box<dyn SlavePty + Send>>| Mutex::new(value))]
    slave: Mutex<Option<Box<dyn SlavePty + Send>>>,
    #[builder(required)]
    reader: Option<JoinHandle<()>>,
}
pub(in crate::engine::runtime::session::manager) use input::KeyboardWriteFailure;
impl ShellSession {
    pub(super) fn cwd(&self) -> PathBuf {
        self.cwd.lock().clone()
    }
    pub(super) fn command_root(&self) -> &Path {
        &self.command_root
    }
    pub(super) fn command_watch(&self) -> Arc<PathWatch> {
        Arc::clone(&self.command_watch)
    }
    pub(super) fn set_cwd(&self, cwd: PathBuf) {
        *self.cwd.lock() = cwd;
    }
    pub(super) fn screen_contents(&self) -> bytestring::ByteString {
        self.screen.contents()
    }
    pub(super) fn model_title(&self) -> String {
        self.screen.model_title()
    }
    pub(super) fn capture_command(
        &self,
        command_id: &str,
        separate_output: bool,
    ) -> Result<Arc<CommandCapture>> {
        self.screen.capture_command(command_id, separate_output)
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
    pub(super) fn exit_code(&self) -> Result<Option<i32>> {
        Ok(self
            .child
            .lock()
            .try_wait()
            .context("failed to inspect shell exit status")?
            .map(|status| status.exit_code().cast_signed()))
    }
    pub(super) fn write_invocation(
        &self,
        command_id: &str,
        command: &str,
        record: &CommandRecord,
        separate_output: bool,
    ) -> Result<()> {
        let choice = self.current_choice();
        let command_source = choice.command_source(command);
        fs_err::write(&record.source, command_source)?;
        let cwd = crate::text::path_text(&self.cwd(), "command working directory")?;
        let dispatch = sonic_rs::to_vec(&crate::contract::CommandDispatch {
            command_id,
            working_directory: &cwd,
            separate_output,
        })
        .context("failed to encode command dispatch")?;
        crate::publication::write_replace(&self.dispatch_file, dispatch)
            .context("failed to publish command dispatch")?;
        let Some(invocation) = choice.invocation_for(&record.directory, separate_output)? else {
            return Ok(());
        };
        let invocation_bytes = invocation.into_bytes();
        let mut writer = self.writer.lock();
        writer
            .write_all(&invocation_bytes)
            .context("failed to write command invocation")?;
        writer.flush().context("failed to flush command invocation")
    }
    pub(super) async fn wait_for_command_start(&self, command: &ManagedCommand) -> Result<()> {
        if command.wait_started(self.command_start_timeout).await? {
            return Ok(());
        }
        anyhow::bail!(
            "shell did not start command within {:?}",
            self.command_start_timeout
        );
    }
    pub(super) async fn wait_for_output(&self, revision: u64, timeout: Duration) -> Result<()> {
        self.screen.wait_for_visible_change(revision, timeout).await
    }
    pub(super) async fn wait_for_exit(&self) -> Result<()> {
        self.screen.wait_for_exit().await
    }
    pub(super) async fn wait_for_terminal_drain(&self) -> Result<()> {
        self.screen.wait_reader_closed().await
    }
    pub(super) fn complete_interruption(&self, command: &ManagedCommand, cwd: &str) -> Result<()> {
        command.publish_interruption(cwd)?;
        self.screen.complete_at_prompt(command.id())
    }
}
