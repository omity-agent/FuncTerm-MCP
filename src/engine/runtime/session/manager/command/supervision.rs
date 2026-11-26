use super::super::tab::Tab;
use super::ManagedCommand;
use crate::runtime::session::observation::PathEvents;
use crate::runtime::session::records::remove_record_directory;
use crate::runtime::session::terminal::Terminal;
use alloc::sync::Arc;
use anyhow::Result;
use core::time::Duration;
impl Tab {
    pub(super) fn supervise(
        self: &Arc<Self>,
        terminal: Arc<Terminal>,
        events: PathEvents,
        command: Arc<ManagedCommand>,
    ) {
        let tab = Arc::clone(self);
        self.supervisors.spawn(async move {
            let outcome = tokio::select! {
                biased;
                settled = command.wait(Duration::MAX) => settled,
                observed = tab.observe_completion(&command) => observed,
            };
            if let Err(error) = outcome {
                tracing::error!(command_id = command.id(), "command supervision failed");
                tracing::debug!(
                    error = format!("{error:#}"),
                    "command supervision failure details"
                );
                if let Err(failure) = tab.stop_after_failure(Arc::clone(&command)).await {
                    command.mark_unavailable(&failure);
                    tracing::error!(
                        command_id = command.id(),
                        "failed to stop command after supervision failure"
                    );
                    tracing::debug!(
                        error = format!("{failure:#}"),
                        "command shutdown failure details"
                    );
                }
            }
            let command_id = command.id().to_owned();
            if let Err(error) = tab
                .cleanup_command_record(command, &terminal, &events)
                .await
            {
                tracing::error!(%command_id, "failed to clean command record");
                tracing::debug!(
                    error = format!("{error:#}"),
                    "command record cleanup failure details"
                );
            }
        });
    }
    async fn observe_completion(self: &Arc<Self>, command: &Arc<ManagedCommand>) -> Result<()> {
        command.wait_for_start().await?;
        let prompt_cwd = tokio::select! {
            biased;
            captured = command.capture.wait_finished() => {
                captured?;
                None
            }
            prompted = command.interrupted_prompt() => {
                let cwd = prompted?;
                if cwd.is_none() {
                    command.capture.wait_finished().await?;
                }
                cwd
            }
        };
        let finished = Arc::clone(command);
        self.with_operation(move |tab| {
            if finished.is_settled() {
                return Ok(());
            }
            let Some(session) = tab.active_session()? else {
                return Ok(());
            };
            if let Some(cwd) = prompt_cwd
                && !finished.capture.finished()?
            {
                session.complete_interruption(&finished, &cwd)?;
            }
            tab.settle_reported(&session, &finished)
        })
        .await
    }
    async fn cleanup_command_record(
        self: &Arc<Self>,
        command: Arc<ManagedCommand>,
        terminal: &Terminal,
        events: &PathEvents,
    ) -> Result<()> {
        tokio::select! {
            biased;
            stopped = terminal.wait_for_shutdown() => return stopped,
            released = command.wait_released(events) => released?,
        }
        self.with_operation(move |tab| {
            if tab.has_session() {
                remove_record_directory(command.record())?;
            }
            Ok(())
        })
        .await
    }
}
