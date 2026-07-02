use super::{Manager, tabs::Tab};
use crate::runtime::protocol::{EndReason, ViewResult};
use crate::runtime::session::records::{
    CommandRecord, command_query, create_record, wait_for_done,
};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
use std::thread;
#[derive(Clone)]
pub(super) struct StoredCommand {
    record: CommandRecord,
}
pub(super) struct StartedCommand {
    command_id: String,
    tab: Arc<Tab>,
    record: CommandRecord,
}
impl Manager {
    pub(crate) fn manual_write(&self, tab_id: &str, bytes: &[u8]) -> Result<()> {
        self.tabs.manual_write(tab_id, bytes)
    }
    pub(crate) fn send_command(
        self: &Arc<Self>,
        tab_id: &str,
        command: &str,
        waiting: Duration,
    ) -> Result<(String, EndReason, ViewResult)> {
        self.tabs.send_command(tab_id, command, waiting)
    }
}
impl StoredCommand {
    pub(super) const fn new(record: CommandRecord) -> Self {
        Self { record }
    }
    pub(super) fn record(&self) -> CommandRecord {
        self.record.clone()
    }
}
impl Tab {
    pub(super) fn manual_write(&self, bytes: &[u8]) -> Result<()> {
        let session = self.live_session()?;
        if !session.is_alive()? {
            self.close_session(&session)?;
            anyhow::bail!("tab id {} was generated, but its shell is gone", self.id());
        }
        session.refresh_choice()?;
        if let Err(error) = session.write_keyboard(bytes) {
            self.close_session(&session)?;
            return Err(error);
        }
        Ok(())
    }
    pub(super) fn start_command(
        self: &Arc<Self>,
        command_id: String,
        command: &str,
    ) -> Result<StartedCommand> {
        let session = self.live_session()?;
        if !session.is_alive()? {
            self.close_session(&session)?;
            anyhow::bail!("tab id {} was generated, but its shell is gone", self.id());
        }
        session.refresh_choice()?;
        session.set_last_command(command.to_owned())?;
        self.remember(&session)?;
        session.reserve(&command_id)?;
        let initial_cwd = session.cwd()?;
        let record = create_record(session.command_root(), &command_id, &initial_cwd)?;
        self.insert_command(command_id.clone(), record.clone())?;
        if let Err(error) = session.write_invocation(&command_id, command, &record) {
            session.release(&command_id)?;
            self.remove_command(&command_id)?;
            self.close_session(&session)?;
            return Err(error);
        }
        self.start_monitor(command_id.clone(), Arc::clone(&session), record.clone());
        Ok(StartedCommand {
            command_id,
            tab: Arc::clone(self),
            record,
        })
    }
    pub(super) fn command_view(&self, command_id: &str, waiting: Duration) -> Result<ViewResult> {
        let record = self
            .find_command(command_id)?
            .with_context(|| format!("command owner is missing record: {command_id}"))?;
        if wait_for_done(&record.done, waiting)?
            && let Some(session) = self.optional_session()?
        {
            session.update_cwd_from_done(&record)?;
            self.remember(&session)?;
        }
        let fallback_cwd = self.command_fallback_cwd(&record)?;
        command_query(&record, &fallback_cwd)
    }
    pub(super) fn find_command_for_view(&self, command_id: &str) -> Result<Option<CommandRecord>> {
        self.find_command(command_id)
    }
    fn start_monitor(
        self: &Arc<Self>,
        command_id: String,
        session: Arc<super::session::ShellSession>,
        record: CommandRecord,
    ) {
        let tab = Arc::clone(self);
        thread::spawn(move || {
            if let Err(error) = wait_for_done(&record.done, Duration::MAX) {
                eprintln!("{error:#}");
                return;
            }
            if let Err(error) = session.update_cwd_from_done(&record) {
                eprintln!("{error:#}");
            }
            if let Err(error) = tab.remember(&session) {
                eprintln!("{error:#}");
            }
            if let Err(error) = session.release(&command_id) {
                eprintln!("{error:#}");
            }
            if let Err(error) = tab.insert_command(command_id, record) {
                eprintln!("{error:#}");
            }
        });
    }
}
impl StartedCommand {
    pub(super) fn wait(self, waiting: Duration) -> Result<(String, EndReason, ViewResult)> {
        let ended = wait_for_done(&self.record.done, waiting)?;
        let reason = if ended {
            if let Some(session) = self.tab.optional_session()? {
                session.update_cwd_from_done(&self.record)?;
                self.tab.remember(&session)?;
            }
            EndReason::CommandEnded
        } else {
            EndReason::WaitTimeout
        };
        let fallback_cwd = self.tab.command_fallback_cwd(&self.record)?;
        let query = command_query(&self.record, &fallback_cwd)?;
        Ok((self.command_id, reason, query))
    }
}
