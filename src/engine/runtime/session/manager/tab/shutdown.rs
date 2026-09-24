use super::Tab;
use crate::runtime::session::manager::command::CommandWait;
use anyhow::{Context as _, Result};
use core::time::Duration;
impl Tab {
    pub(super) fn close(&self) -> Result<()> {
        let _operation = self.operation.lock();
        let Some(session) = self.optional_session() else {
            return Ok(());
        };
        let command = if let Some(id) = session.busy_command_id() {
            Some(
                self.find_command(&id)
                    .with_context(|| format!("Tab is missing its running command {id}"))?,
            )
        } else {
            None
        };
        if let Some(active) = command.as_ref() {
            active.cancel_title_capture()?;
        }
        session.terminate()?;
        if let Some(active) = command {
            match active.wait(Duration::ZERO)? {
                CommandWait::Finished => self.finish_done_command(&active)?,
                CommandWait::Running => active.mark_failed("Tab was closed")?,
                CommandWait::Failed => {}
            }
            session.release(active.id());
        }
        self.close_session(&session)
    }
}
