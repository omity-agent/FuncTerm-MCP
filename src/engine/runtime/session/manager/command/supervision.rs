use super::super::tab::Tab;
use super::ManagedCommand;
use super::settlement::wait_for_resumption;
use crate::contract::{CommandOutcome, CommandReport};
use crate::runtime::session::observation::PathEvents;
use crate::runtime::session::records::remove_record_directory;
use crate::runtime::session::terminal::{ShellPrompt, Terminal};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
use std::path::{Path, PathBuf};
enum Boundary {
    Reported {
        report: CommandReport<String>,
        parent_prompt: Option<Arc<ShellPrompt>>,
    },
    Interrupted(String),
}
impl Tab {
    pub(super) fn supervise(
        self: &Arc<Self>,
        terminal: Arc<Terminal>,
        events: PathEvents,
        tab_root: PathBuf,
        command: Arc<ManagedCommand>,
    ) {
        let tab = Arc::clone(self);
        self.supervisors.spawn(async move {
            let observation = tokio::select! {
                biased;
                settled = command.wait(Duration::MAX) => settled.map(|()| None),
                observed = observe_boundary(&command, &terminal, &events, &tab_root) => observed.map(Some),
            };
            let outcome = match observation {
                Ok(Some(boundary)) => tab.settle_observed(Arc::clone(&command), boundary).await,
                Ok(None) => return,
                Err(error) => Err(error),
            };
            let observed = match outcome {
                Ok(Some(observed)) => observed,
                Ok(None) => return,
                Err(error) => {
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
                    return;
                }
            };
            let command_id = command.id().to_owned();
            if let Err(error) = tab
                .cleanup_command_record(command, observed, &terminal, &events)
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
    async fn settle_observed(
        self: &Arc<Self>,
        command: Arc<ManagedCommand>,
        boundary: Boundary,
    ) -> Result<Option<CommandOutcome>> {
        self.with_operation(move |tab| {
            if command.is_settled() {
                return Ok(None);
            }
            let Some(session) = tab.active_session()? else {
                return Ok(None);
            };
            let completion = match boundary {
                Boundary::Reported {
                    report,
                    parent_prompt,
                } => {
                    if let Some(prompt) = parent_prompt {
                        session.resume_at_prompt(&prompt)?;
                    }
                    report
                }
                Boundary::Interrupted(cwd) => {
                    session
                        .terminal()
                        .complete_at_prompt(command.id(), &command.capture)?;
                    match command.read_completion()? {
                        Some(published) => published,
                        None => command.interruption_report(cwd)?,
                    }
                }
            };
            let outcome = completion.outcome;
            tab.settle_reported(&session, &command, completion)?;
            Ok(Some(outcome))
        })
        .await
    }
    async fn cleanup_command_record(
        self: &Arc<Self>,
        command: Arc<ManagedCommand>,
        outcome: CommandOutcome,
        terminal: &Terminal,
        events: &PathEvents,
    ) -> Result<()> {
        if matches!(outcome, CommandOutcome::HandedOff) {
            tokio::select! {
                biased;
                stopped = terminal.wait_for_shutdown() => return stopped,
                released = command.wait_released(events) => released?,
            }
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
async fn observe_boundary(
    command: &ManagedCommand,
    terminal: &Terminal,
    events: &PathEvents,
    tab_root: &Path,
) -> Result<Boundary> {
    command.wait_for_start().await?;
    tokio::select! {
        biased;
        captured = command.capture.wait_finished() => captured?,
        prompted = command.interrupted_prompt() => {
            if let Some(cwd) = prompted? {
                return Ok(Boundary::Interrupted(cwd));
            }
            command.capture.wait_finished().await?;
        }
    }
    let report = command
        .read_completion()?
        .context("completed command did not report its result")?;
    let parent_prompt = if !command.frame.is_root() && report.outcome.exits_shell() {
        wait_for_resumption(command, terminal, events, tab_root).await?
    } else {
        None
    };
    Ok(Boundary::Reported {
        report,
        parent_prompt,
    })
}
