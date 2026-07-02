use super::lifecycle::{release_shell, reserve_shell};
use super::{Manager, ShellSession};
use crate::runtime::protocol::{EndReason, QueryResult};
use crate::runtime::session::records::{
    CommandRecord, command_query, create_record, wait_for_done,
};
use crate::runtime::session::support::lock_mutex;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use base64_turbo::STANDARD;
use core::time::Duration;
use std::fs;
use std::io::Write as _;
use std::thread;
impl Manager {
    pub(crate) fn manual_write(&self, tab_id: &str, bytes: &[u8]) -> Result<()> {
        let shell = self.shell(tab_id)?;
        self.ensure_shell_running(tab_id, &shell)?;
        let keyboard_bytes = shell.choice.keyboard_bytes(bytes);
        let write_result = {
            let mut writer = lock_mutex(&shell.writer, "writer")?;
            writer
                .write_all(&keyboard_bytes)
                .context("failed to write to pty")
                .and_then(|()| writer.flush().context("failed to flush pty writer"))
        };
        if let Err(error) = write_result {
            self.remove_shell(tab_id)?;
            return Err(error);
        }
        Ok(())
    }
    pub(crate) fn send_command(
        self: &Arc<Self>,
        tab_id: &str,
        command: &str,
        waiting: Duration,
    ) -> Result<(String, EndReason, QueryResult)> {
        let shell = self.shell(tab_id)?;
        self.ensure_shell_running(tab_id, &shell)?;
        let command_id = self.next_command_id()?;
        reserve_shell(&shell, &command_id)?;
        let initial_cwd = Self::shell_cwd(&shell)?;
        let record = create_record(&shell.command_root, &command_id, tab_id, &initial_cwd)?;
        lock_mutex(&self.commands, "command")?.insert(command_id.clone(), record.clone());
        if let Err(error) = Self::write_invocation(&shell, &command_id, command, &record) {
            release_shell(&shell, &command_id)?;
            lock_mutex(&self.commands, "command")?.remove(&command_id);
            self.remove_shell(tab_id)?;
            return Err(error);
        }
        self.start_monitor(command_id.clone(), Arc::clone(&shell), record.clone());
        let ended = wait_for_done(&record.done, waiting)?;
        let reason = if ended {
            Self::update_shell_cwd(&shell, &record)?;
            EndReason::CommandEnded
        } else {
            EndReason::WaitTimeout
        };
        let cwd = Self::shell_cwd(&shell)?;
        let query = command_query(&record, &cwd)?;
        Ok((command_id, reason, query))
    }
    fn shell(&self, tab_id: &str) -> Result<Arc<ShellSession>> {
        self.find_shell(tab_id)?
            .with_context(|| format!("unknown tab id {tab_id}"))
    }
    fn remove_shell(&self, tab_id: &str) -> Result<()> {
        lock_mutex(&self.shells, "shell")?.remove(tab_id);
        Ok(())
    }
    fn ensure_shell_running(&self, tab_id: &str, shell: &ShellSession) -> Result<()> {
        let status = lock_mutex(&shell.child, "child")?
            .try_wait()
            .context("failed to poll shell child")?;
        if let Some(exit_status) = status {
            self.remove_shell(tab_id)?;
            bail!("shell process exited with status {exit_status}");
        }
        Ok(())
    }
    fn write_invocation(
        shell: &ShellSession,
        command_id: &str,
        command: &str,
        record: &CommandRecord,
    ) -> Result<()> {
        let payload = STANDARD.encode(command.as_bytes());
        fs::write(&record.payload, payload).context("failed to write command payload")?;
        let directory = record
            .stdout
            .parent()
            .context("missing command directory")?;
        let line = shell
            .choice
            .invocation(command_id, directory, &record.initial_cwd);
        let mut writer = lock_mutex(&shell.writer, "writer")?;
        writer
            .write_all(line.as_bytes())
            .context("failed to write command invocation")?;
        writer.flush().context("failed to flush command invocation")
    }
    fn start_monitor(
        self: &Arc<Self>,
        command_id: String,
        shell: Arc<ShellSession>,
        record: CommandRecord,
    ) {
        let manager = Arc::clone(self);
        thread::spawn(move || {
            if let Err(error) = wait_for_done(&record.done, Duration::MAX) {
                eprintln!("{error:#}");
                return;
            }
            if let Err(error) = Self::update_shell_cwd(&shell, &record) {
                eprintln!("{error:#}");
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
