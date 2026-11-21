use super::super::{shell_session::ShellSession, tab::Tab};
use super::{ManagedCommand, SessionEnd};
use crate::runtime::session::records::remove_record_directory;
use alloc::sync::Arc;
use anyhow::Result;
impl Tab {
    pub(super) fn supervise(
        self: &Arc<Self>,
        session: Arc<ShellSession>,
        command: Arc<ManagedCommand>,
    ) {
        let tab = Arc::clone(self);
        self.supervisors.spawn(async move {
            if let Err(error) = tab.observe_completion(&session, &command).await {
                tracing::error!(command_id = command.id(), "command supervision failed");
                tracing::debug!(
                    error = format!("{error:#}"),
                    "command supervision failure details"
                );
                if let Err(failure) = tab
                    .stop_after_failure(Arc::clone(&session), Arc::clone(&command))
                    .await
                {
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
            if let Err(error) = tab.cleanup_command_record(command, session).await {
                tracing::error!(%command_id, "failed to clean command record");
                tracing::debug!(
                    error = format!("{error:#}"),
                    "command record cleanup failure details"
                );
            }
        });
    }
    async fn observe_completion(
        self: &Arc<Self>,
        session: &Arc<ShellSession>,
        command: &Arc<ManagedCommand>,
    ) -> Result<()> {
        tokio::select! {
            biased;
            exited = session.wait_for_exit() => {
                exited?;
                return self.finish_exited(session).await;
            }
            started = session.wait_for_command_start(command) => started?,
        }
        tokio::select! {
            biased;
            exited = session.wait_for_exit() => {
                exited?;
                self.finish_exited(session).await
            }
            captured = command.capture.wait_finished() => {
                captured?;
                self.finish_reported(session, command, None).await
            }
            prompted = command.interrupted_prompt() => {
                if let Some(cwd) = prompted? {
                    self.finish_reported(session, command, Some(cwd)).await
                } else {
                    command.capture.wait_finished().await?;
                    self.finish_reported(session, command, None).await
                }
            }
        }
    }
    async fn finish_exited(self: &Arc<Self>, session: &Arc<ShellSession>) -> Result<()> {
        let target = Arc::clone(session);
        self.with_operation(move |tab| {
            if tab.optional_session().is_some() {
                let code = target.exit_code()?.ok_or_else(|| {
                    anyhow::anyhow!("shell exit was observed without an exit status")
                })?;
                tab.end_session(&target, SessionEnd::Exited(code))?;
            }
            Ok(())
        })
        .await
    }
    async fn finish_reported(
        self: &Arc<Self>,
        session: &Arc<ShellSession>,
        command: &Arc<ManagedCommand>,
        prompt_cwd: Option<String>,
    ) -> Result<()> {
        let target = Arc::clone(session);
        let finished = Arc::clone(command);
        self.with_operation(move |tab| {
            if finished.is_settled() {
                return Ok(());
            }
            if let Some(cwd) = prompt_cwd
                && !finished.capture.finished()?
            {
                target.complete_interruption(&finished, &cwd)?;
            }
            tab.settle_reported(&target, &finished)
        })
        .await
    }
    async fn cleanup_command_record(
        &self,
        command: Arc<ManagedCommand>,
        session: Arc<ShellSession>,
    ) -> Result<()> {
        let remove_record = tokio::select! {
            biased;
            stopped = session.wait_for_shutdown() => {
                stopped?;
                false
            }
            released = command.wait_released(session.command_watch()) => {
                released?;
                true
            }
        };
        self.blocking
            .run(move || {
                let removed = if remove_record {
                    remove_record_directory(command.record())
                } else {
                    Ok(())
                };
                drop(session);
                removed
            })
            .await
    }
}
