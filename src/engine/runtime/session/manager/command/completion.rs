use super::lifecycle::{ManagedCommand, SettledCommand};
use crate::contract::{CommandOutcome, CommandReport};
use crate::runtime::protocol::{CommandSnapshot, EndReason};
use crate::runtime::session::records::command_note;
use anyhow::{Result, ensure};
use std::path::PathBuf;
#[derive(Clone, Copy)]
pub(super) enum Completion<'report> {
    Reported(&'report CommandReport<String>),
    Exited(i32),
    Closed,
    Failed,
}
impl ManagedCommand {
    pub(super) fn interruption_report(&self, cwd: String) -> Result<CommandReport<String>> {
        ensure!(
            self.state.lock().input.interrupted(),
            "shell returned to its prompt without publishing command completion"
        );
        Ok(CommandReport {
            command_id: self.id().to_owned(),
            elapsed: None,
            cwd,
            outcome: CommandOutcome::Returned {
                exit_code: 130,
                shell_exit: false,
            },
        })
    }
    pub(super) fn view(&self) -> Result<(EndReason, CommandSnapshot)> {
        let mut state = self.state.lock();
        if let Some(outcome) = state.outcome.as_ref() {
            let result = outcome
                .as_ref()
                .map_err(|message| anyhow::anyhow!("{message}"))?;
            return Ok((result.reason, result.view.clone()));
        }
        let snapshot = self.snapshot(&mut state, self.capture.title(), None, false)?;
        drop(state);
        Ok((EndReason::WaitTimeout, snapshot))
    }
    pub(super) fn settle(
        &self,
        completion: Completion<'_>,
        update_cwd: impl FnOnce(PathBuf),
    ) -> Result<()> {
        let mut state = self.state.lock();
        if state.outcome.is_some() {
            return Ok(());
        }
        let title = if matches!(completion, Completion::Reported(_)) {
            self.capture.title()
        } else {
            self.capture.cancel()?
        };
        let on_shutdown;
        let report = match completion {
            Completion::Reported(report) => Some(report),
            Completion::Failed => None,
            Completion::Exited(_) | Completion::Closed => {
                on_shutdown = self.read_completion()?;
                on_shutdown.as_ref()
            }
        };
        let mut view = self.snapshot(&mut state, title, report, true)?;
        let reason = if let Some(result) = report {
            update_cwd(PathBuf::from(&result.cwd));
            state.input.normalize(&mut view);
            EndReason::CommandEnded
        } else if let Completion::Exited(exit_code) = completion {
            view.command.exit_code = Some(exit_code);
            state.input.normalize(&mut view);
            EndReason::CommandEnded
        } else {
            view.command.exit_code = Some(1_i32);
            let (reason, note) = if matches!(completion, Completion::Closed) {
                (EndReason::TabClosed, "Tab was closed")
            } else {
                (
                    EndReason::CommandFailed,
                    "FuncTerm could not complete the command",
                )
            };
            view.note = command_note(&view.command.output, note);
            reason
        };
        view.command.finished = true;
        state.finish(Ok(SettledCommand { reason, view }));
        drop(state);
        self.settled.notify(usize::MAX);
        Ok(())
    }
    pub(super) fn mark_unavailable(&self, error: &anyhow::Error) {
        let mut state = self.state.lock();
        if state.outcome.is_none() {
            state.finish(Err(format!("{error:#}")));
            drop(state);
            self.settled.notify(usize::MAX);
        }
    }
    pub(super) fn read_completion(&self) -> Result<Option<CommandReport<String>>> {
        let report = CommandReport::read(&self.record.directory)?;
        if let Some(record) = report.as_ref() {
            ensure!(
                record.command_id == self.id(),
                "shell reported completion for a different command"
            );
        }
        Ok(report)
    }
}
