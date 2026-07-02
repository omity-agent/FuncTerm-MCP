use super::lifecycle::{release_shell, reserve_shell};
use super::{Manager, ShellSession};
use crate::runtime::protocol::{EndReason, ViewResult};
use crate::runtime::session::records::{
    CommandRecord, command_query, create_record, wait_for_done,
};
use crate::runtime::session::support::lock_mutex;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use base64_turbo::STANDARD;
use core::time::Duration;
use std::collections::HashMap;
use std::fs;
use std::io::Write as _;
use std::sync::Mutex;
use std::thread;
#[derive(Default)]
pub(super) struct CommandRegistry {
    records: Mutex<HashMap<String, CommandRecord>>,
}
impl CommandRegistry {
    pub(super) fn insert(&self, command_id: String, record: CommandRecord) -> Result<()> {
        lock_mutex(&self.records, "command")?.insert(command_id, record);
        Ok(())
    }
    pub(super) fn remove(&self, command_id: &str) -> Result<Option<CommandRecord>> {
        Ok(lock_mutex(&self.records, "command")?.remove(command_id))
    }
    pub(super) fn find(&self, command_id: &str) -> Result<Option<CommandRecord>> {
        Ok(lock_mutex(&self.records, "command")?
            .get(command_id)
            .cloned())
    }
    pub(super) fn id_exists(&self, command_id: &str) -> Result<bool> {
        Ok(lock_mutex(&self.records, "command")?.contains_key(command_id))
    }
    pub(super) fn insert_if_absent(&self, command_id: String, record: CommandRecord) -> Result<()> {
        lock_mutex(&self.records, "command")?
            .entry(command_id)
            .or_insert(record);
        Ok(())
    }
}
impl Manager {
    pub(crate) fn manual_write(&self, tab_id: &str, bytes: &[u8]) -> Result<()> {
        let shell = self.shell(tab_id)?;
        self.ensure_shell_running(tab_id, &shell)?;
        shell.refresh_choice()?;
        let choice = shell.current_choice()?;
        let keyboard_bytes = choice.keyboard_bytes(bytes);
        let write_result = Self::write_keyboard(&shell, &keyboard_bytes);
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
    ) -> Result<(String, EndReason, ViewResult)> {
        let shell = self.shell(tab_id)?;
        self.ensure_shell_running(tab_id, &shell)?;
        shell.refresh_choice()?;
        *lock_mutex(&shell.last_command, "last command")? = Some(command.to_owned());
        self.remember_tab(tab_id, &shell)?;
        let command_id = self.next_command_id()?;
        reserve_shell(&shell, &command_id)?;
        let initial_cwd = shell.cwd()?;
        let record = create_record(&shell.command_root, &command_id, tab_id, &initial_cwd)?;
        self.commands.insert(command_id.clone(), record.clone())?;
        if let Err(error) = Self::write_invocation(&shell, &command_id, command, &record) {
            release_shell(&shell, &command_id)?;
            self.commands.remove(&command_id)?;
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
        let cwd = shell.cwd()?;
        let query = command_query(&record, &cwd)?;
        Ok((command_id, reason, query))
    }
    fn shell(&self, tab_id: &str) -> Result<Arc<ShellSession>> {
        if let Some(shell) = self.find_shell(tab_id)? {
            return Ok(shell);
        }
        if self.generated_tab_id_exists(tab_id)? {
            bail!("tab id {tab_id} was generated, but its shell is gone");
        }
        bail!("unknown tab id {tab_id}")
    }
    fn remove_shell(&self, tab_id: &str) -> Result<()> {
        let removed_shell = self.tabs.remove_shell(tab_id)?;
        if let Some(shell) = removed_shell {
            self.remember_tab(tab_id, &shell)?;
        }
        Ok(())
    }
    fn ensure_shell_running(&self, tab_id: &str, shell: &ShellSession) -> Result<()> {
        if !shell.is_alive()? {
            self.remove_shell(tab_id)?;
            bail!("tab id {tab_id} was generated, but its shell is gone");
        }
        Ok(())
    }
    fn write_keyboard(shell: &ShellSession, keyboard_bytes: &[u8]) -> Result<()> {
        let mut writer = lock_mutex(&shell.writer, "writer")?;
        writer
            .write_all(keyboard_bytes)
            .context("failed to write to pty")?;
        writer.flush().context("failed to flush pty writer")?;
        drop(writer);
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
            .current_choice()?
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
            if let Err(error) = manager.remember_tab(&record.tab_id, &shell) {
                eprintln!("{error:#}");
            }
            if let Ok(mut busy) = shell.busy.lock() {
                *busy = None;
            }
            if let Err(error) = manager.commands.insert_if_absent(command_id, record) {
                eprintln!("{error:#}");
            }
        });
    }
}
