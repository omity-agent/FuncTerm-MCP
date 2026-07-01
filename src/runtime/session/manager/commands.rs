use super::lifecycle::{release_shell, reserve_shell};
use super::{Manager, ShellSession};
use crate::runtime::protocol::{EndReason, QueryResult};
use crate::runtime::session::records::{
    CommandRecord, command_query, create_record, wait_for_done,
};
use crate::runtime::session::support::lock_mutex;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use std::io::Write as _;
use std::thread;
impl Manager {
    pub(crate) fn write_keyboard(&self, shell_id: &str, bytes: &[u8]) -> Result<()> {
        let shell = self.shell(shell_id)?;
        self.ensure_shell_running(shell_id, &shell)?;
        let write_result = {
            let mut writer = lock_mutex(&shell.writer, "writer")?;
            writer
                .write_all(bytes)
                .context("failed to write to pty")
                .and_then(|()| writer.flush().context("failed to flush pty writer"))
        };
        if let Err(error) = write_result {
            self.remove_shell(shell_id)?;
            return Err(error);
        }
        Ok(())
    }
    pub(crate) fn send_command(
        self: &Arc<Self>,
        shell_id: &str,
        command: &str,
        wait_ms: u64,
    ) -> Result<(String, EndReason, QueryResult)> {
        let shell = self.shell(shell_id)?;
        self.ensure_shell_running(shell_id, &shell)?;
        let command_id = self.next_command_id()?;
        reserve_shell(&shell, &command_id)?;
        let initial_cwd = Self::shell_cwd(&shell)?;
        let record = create_record(&shell.command_root, &command_id, shell_id, &initial_cwd)?;
        lock_mutex(&self.commands, "command")?.insert(command_id.clone(), record.clone());
        if let Err(error) = Self::write_invocation(&shell, &command_id, command, &record) {
            release_shell(&shell, &command_id)?;
            lock_mutex(&self.commands, "command")?.remove(&command_id);
            self.remove_shell(shell_id)?;
            return Err(error);
        }
        self.start_monitor(command_id.clone(), Arc::clone(&shell), record.clone());
        let ended = wait_for_done(&record.done, Duration::from_millis(wait_ms))?;
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
    fn shell(&self, shell_id: &str) -> Result<Arc<ShellSession>> {
        self.find_shell(shell_id)?
            .with_context(|| format!("unknown shell id {shell_id}"))
    }
    fn remove_shell(&self, shell_id: &str) -> Result<()> {
        lock_mutex(&self.shells, "shell")?.remove(shell_id);
        Ok(())
    }
    fn ensure_shell_running(&self, shell_id: &str, shell: &ShellSession) -> Result<()> {
        let status = lock_mutex(&shell.child, "child")?
            .try_wait()
            .context("failed to poll shell child")?;
        if let Some(exit_status) = status {
            self.remove_shell(shell_id)?;
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
        let directory = record
            .stdout
            .parent()
            .context("missing command directory")?;
        let line = shell
            .choice
            .invocation(command_id, command, directory, &record.initial_cwd);
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
