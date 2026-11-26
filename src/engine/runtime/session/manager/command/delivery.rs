use super::super::shell_session::ShellSession;
use super::super::tab::Tab;
use super::SessionEnd;
use crate::runtime::protocol::{KeyboardInput, RequestRejected, ViewResult};
use alloc::sync::Arc;
use anyhow::Result;
use core::time::Duration;
impl Tab {
    pub(in crate::engine::runtime::session::manager) async fn manual_write(
        self: &Arc<Self>,
        input: KeyboardInput,
        wait_timeout: Duration,
    ) -> Result<ViewResult> {
        let busy = self
            .observe_session(ShellSession::busy_command)
            .ok_or_else(|| self.closed_tab())?;
        let expected = busy.ok_or_else(|| {
            RequestRejected::new(
                "manual_write is unavailable while the prompt is idle; use run for prompt commands",
            )
        })?;
        expected.wait_for_start().await?;
        let (terminal, revision) = self
            .with_operation(move |tab| {
                let target = tab.live_session()?;
                target.refresh_route()?;
                match target.write_keyboard_for_running_command(&expected, &input) {
                    Err(error) if !error.is::<RequestRejected>() => {
                        if let Err(cleanup) = tab.end_session(&target, SessionEnd::Failed) {
                            tracing::error!(
                                "failed to close the Tab after keyboard delivery failed"
                            );
                            tracing::debug!(
                                error = format!("{cleanup:#}"),
                                "shell shutdown failure details"
                            );
                        }
                        Err(error)
                    }
                    result => Ok((target.terminal(), result?)),
                }
            })
            .await?;
        terminal
            .wait_for_visible_change(revision, wait_timeout)
            .await?;
        self.with_operation(|tab| tab.tab_view()).await
    }
}
