use super::{Manager, ShellSession};
use crate::runtime::protocol::ViewResult;
use crate::runtime::session::records::{CommandRecord, command_query, wait_for_done};
use crate::runtime::session::support::lock_mutex;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use std::thread;
impl Manager {
    pub(crate) fn view(&self, id: &str, waiting: core::time::Duration) -> Result<ViewResult> {
        if let Some(shell) = self.find_shell(id)? {
            self.wait_for_shell_view(&shell, waiting)?;
            return self.tab_view(id, &shell);
        }
        if let Some(snapshot) = self.find_tab_snapshot(id)? {
            return Ok(snapshot.into_view(false));
        }
        if let Some(record) = self.find_command(id)? {
            self.wait_for_command_view(&record, waiting)?;
            return self.command_view(&record);
        }
        bail!("unknown id {id}")
    }
    fn tab_view(&self, tab_id: &str, shell: &Arc<ShellSession>) -> Result<ViewResult> {
        let alive = Self::shell_alive(shell)?;
        Self::refresh_shell_choice(shell)?;
        let snapshot = self.remember_tab(tab_id, shell)?;
        Ok(snapshot.into_view(alive))
    }
    fn command_view(&self, record: &CommandRecord) -> Result<ViewResult> {
        let fallback_cwd = self.command_fallback_cwd(record)?;
        command_query(record, &fallback_cwd)
    }
    fn wait_for_shell_view(
        &self,
        shell: &Arc<ShellSession>,
        waiting: core::time::Duration,
    ) -> Result<()> {
        let busy_command_id = lock_mutex(&shell.busy, "busy")?.clone();
        let Some(command_id) = busy_command_id else {
            thread::sleep(waiting);
            return Ok(());
        };
        let record = self
            .find_command(&command_id)?
            .with_context(|| format!("busy shell command is missing: {command_id}"))?;
        if wait_for_done(&record.done, waiting)? {
            Self::update_shell_cwd(shell, &record)?;
        }
        Ok(())
    }
    fn wait_for_command_view(
        &self,
        record: &CommandRecord,
        waiting: core::time::Duration,
    ) -> Result<()> {
        if wait_for_done(&record.done, waiting)?
            && let Some(shell) = self.find_shell(&record.tab_id)?
        {
            Self::update_shell_cwd(&shell, record)?;
        }
        Ok(())
    }
}
