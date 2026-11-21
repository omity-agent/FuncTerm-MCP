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
        let session = self.live_session()?;
        let expected = session.busy_command().ok_or_else(|| {
            RequestRejected::new(
                "manual_write is unavailable while the prompt is idle; use run for prompt commands",
            )
        })?;
        session.wait_for_command_start(&expected).await?;
        let target = Arc::clone(&session);
        let revision = self
            .with_operation(move |tab| {
                tab.live_session()?;
                if let Some(code) = target.exit_code()? {
                    tab.end_session(&target, SessionEnd::Exited(code))?;
                    return Err(tab.closed_tab().into());
                }
                target.refresh_choice()?;
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
                    result => result,
                }
            })
            .await?;
        session.wait_for_output(revision, wait_timeout).await?;
        self.with_operation(|tab| tab.tab_view()).await
    }
}
