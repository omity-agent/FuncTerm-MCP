mod cleanup;
#[cfg(windows)]
mod containment;
mod foreground;
mod input;
pub(super) mod process;
use super::command::ManagedCommand;
use crate::contract::DISPATCH_FILE;
use crate::runtime::session::observation::{PathEvents, PathWatch};
use crate::runtime::session::records::CommandRecord;
use crate::runtime::session::terminal::{CommandCapture, Terminal};
use crate::shell::DispatchWake;
use crate::shell::frame::{ShellFrame, ShellRoute};
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
    #[builder(with = |value: ShellRoute| Mutex::new(value))]
    route: Mutex<ShellRoute>,
    #[builder(with = |value: PathBuf| Mutex::new(value))]
    cwd: Mutex<PathBuf>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    screen: Arc<Terminal>,
    #[builder(skip)]
    busy: Mutex<Option<Arc<ManagedCommand>>>,
    #[builder(skip)]
    terminated: Mutex<bool>,
    command_root: PathBuf,
    workspace_watch: PathWatch,
    tab_root: PathBuf,
    command_start_timeout: Duration,
    process_tree: process::ProcessTree,
    #[builder(with = |value: Box<dyn Child + Send + Sync>| Mutex::new(value))]
    child: Mutex<Box<dyn Child + Send + Sync>>,
    #[builder(with = |value: Option<Box<dyn SlavePty + Send>>| Mutex::new(value))]
    slave: Mutex<Option<Box<dyn SlavePty + Send>>>,
    #[builder(with = |value: JoinHandle<()>| Mutex::new(Some(value)))]
    reader: Mutex<Option<JoinHandle<()>>>,
    #[builder(required)]
    directory: Option<tempfile::TempDir>,
}
impl ShellSession {
    pub(super) fn cwd(&self) -> PathBuf {
        self.cwd.lock().clone()
    }
    pub(super) fn command_root(&self) -> &Path {
        &self.command_root
    }
    pub(super) fn workspace_events(&self) -> PathEvents {
        self.workspace_watch.events()
    }
    pub(super) fn tab_root(&self) -> &Path {
        &self.tab_root
    }
    pub(super) const fn command_start_timeout(&self) -> Duration {
        self.command_start_timeout
    }
    pub(super) fn terminal(&self) -> Arc<Terminal> {
        Arc::clone(&self.screen)
    }
    pub(super) fn startup_child(&mut self) -> &mut (dyn Child + Send + Sync) {
        self.child.get_mut().as_mut()
    }
    pub(super) fn set_cwd(&self, cwd: PathBuf) {
        *self.cwd.lock() = cwd;
    }
    pub(super) fn screen_contents(&self) -> bytestring::ByteString {
        self.screen.contents()
    }
    pub(super) fn title(&self) -> String {
        self.screen.title()
    }
    pub(super) fn capture_command(
        &self,
        command_id: &str,
        frame: ShellFrame,
        separate_output: bool,
    ) -> Result<Arc<CommandCapture>> {
        self.screen
            .capture_command(command_id, frame.id(), separate_output)
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
        frame: ShellFrame,
        command_id: &str,
        command: &str,
        record: &CommandRecord,
        separate_output: bool,
    ) -> Result<()> {
        let choice = frame.choice;
        let delivery = choice.dispatch_wakeup(&record.directory)?;
        let command_source = choice.command_source(command);
        fs_err::write(&record.source, command_source)?;
        fs_err::write(
            record.directory.join(crate::contract::TITLE_FILE),
            self.title(),
        )
        .context("failed to write command title")?;
        let cwd = crate::text::path_text(&self.cwd(), "command working directory")?;
        let dispatch = sonic_rs::to_vec(&crate::contract::CommandDispatch {
            command_id,
            working_directory: &cwd,
            separate_output,
        })
        .context("failed to encode command dispatch")?;
        let dispatch_file = frame.state_directory(&self.tab_root).join(DISPATCH_FILE);
        crate::publication::write_replace(&dispatch_file, dispatch)
            .context("failed to publish command dispatch")?;
        let DispatchWake::Keyboard(invocation) = delivery else {
            return Ok(());
        };
        let invocation_bytes = invocation.into_bytes();
        let mut writer = self.writer.lock();
        writer
            .write_all(&invocation_bytes)
            .context("failed to write command invocation")?;
        writer.flush().context("failed to flush command invocation")
    }
}
