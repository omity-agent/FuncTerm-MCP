use super::Tab;
use crate::runtime::protocol::{RequestRejected, ViewResult};
use alloc::sync::Arc;
use anyhow::Result;
use core::time::Duration;
impl Tab {
    pub(super) async fn view(self: &Arc<Self>, wait_timeout: Duration) -> Result<ViewResult> {
        let Some((terminal, busy)) =
            self.observe_session(|session| (session.terminal(), session.busy_command()))
        else {
            return Ok(self.snapshot_view());
        };
        if let Some(command) = busy {
            command.wait(wait_timeout).await?;
        } else if !wait_timeout.is_zero() {
            let deadline = tokio::time::Instant::now()
                .checked_add(wait_timeout)
                .ok_or_else(|| {
                    RequestRejected::new("wait_timeout exceeds the supported timer range")
                })?;
            tokio::select! {
                () = tokio::time::sleep_until(deadline) => {}
                exited = terminal.wait_for_exit() => { exited?; }
            }
        }
        self.with_operation(|tab| tab.tab_view()).await
    }
    pub(in crate::engine::runtime::session::manager) fn tab_view(&self) -> Result<ViewResult> {
        if let Some(session) = self.active_session()? {
            session.refresh_route()?;
            return Ok(self.remember(&session)?.into_view(true));
        }
        Ok(self.snapshot_view())
    }
}
