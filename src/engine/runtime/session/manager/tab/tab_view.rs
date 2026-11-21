use super::super::command::SessionEnd;
use super::Tab;
use crate::runtime::protocol::{RequestRejected, ViewResult};
use alloc::sync::Arc;
use anyhow::Result;
use core::time::Duration;
impl Tab {
    pub(super) async fn view(self: &Arc<Self>, wait_timeout: Duration) -> Result<ViewResult> {
        let Some(session) = self.optional_session() else {
            return Ok(self.snapshot_view());
        };
        if let Some(command) = session.busy_command() {
            command.wait(wait_timeout).await?;
        } else if !wait_timeout.is_zero() {
            let deadline = tokio::time::Instant::now()
                .checked_add(wait_timeout)
                .ok_or_else(|| {
                    RequestRejected::new("wait_timeout exceeds the supported timer range")
                })?;
            tokio::select! {
                () = tokio::time::sleep_until(deadline) => {}
                exited = session.wait_for_exit() => { exited?; }
            }
        }
        drop(session);
        self.with_operation(|tab| tab.tab_view()).await
    }
    pub(in crate::engine::runtime::session::manager) fn tab_view(&self) -> Result<ViewResult> {
        if let Some(session) = self.optional_session() {
            session.refresh_choice()?;
            if let Some(code) = session.exit_code()? {
                self.end_session(&session, SessionEnd::Exited(code))?;
            } else {
                return Ok(self.remember(&session)?.into_view(true));
            }
        }
        Ok(self.snapshot_view())
    }
}
