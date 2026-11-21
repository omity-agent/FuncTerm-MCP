use super::lifecycle::{ManagedCommand, SettledCommand};
use crate::runtime::protocol::{CommandSnapshot, EndReason};
use crate::runtime::session::records::{DoneFile, command_note, read_done};
use anyhow::{Result, ensure};
use std::path::PathBuf;
#[derive(Clone, Copy)]
pub(super) enum Completion {
    Reported,
    Exited(i32),
    Closed,
    Failed,
}
impl ManagedCommand {
    pub(in crate::engine::runtime::session::manager) fn publish_interruption(
        &self,
        cwd: &str,
    ) -> Result<()> {
        if self.record.done.try_exists()? {
            return Ok(());
        }
        ensure!(
            self.state.lock().input.interrupted(),
            "shell returned to its prompt without publishing command completion"
        );
        let duration = format!("{}ns", self.time_consumption().as_nanos());
        let done = crate::contract::CommandCompletion {
            command_id: self.id(),
            exit_code: 130,
            time_consumption: duration.as_str(),
            cwd,
        };
        crate::publication::write_once(&self.record.done, sonic_rs::to_vec(&done)?)?;
        crate::publication::write_once(&self.record.released, b"")?;
        Ok(())
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
        completion: Completion,
        update_cwd: impl FnOnce(PathBuf),
    ) -> Result<bool> {
        let mut state = self.state.lock();
        if state.outcome.is_some() {
            return Ok(false);
        }
        let reported = matches!(completion, Completion::Reported);
        let title = if reported {
            self.capture.title()
        } else {
            self.capture.cancel()?
        };
        let done = if matches!(completion, Completion::Failed) {
            None
        } else {
            self.read_completion()?
        };
        ensure!(
            !reported || done.is_some(),
            "completed command did not report its result"
        );
        let requested_exit = reported && self.record.session_exit.try_exists()?;
        let mut view = self.snapshot(&mut state, title, done.as_ref(), true)?;
        let reason = if let Some(result) = done {
            update_cwd(PathBuf::from(result.cwd));
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
        Ok(requested_exit)
    }
    pub(super) fn mark_unavailable(&self, error: &anyhow::Error) {
        let mut state = self.state.lock();
        if state.outcome.is_none() {
            state.finish(Err(format!("{error:#}")));
            drop(state);
            self.settled.notify(usize::MAX);
        }
    }
    fn read_completion(&self) -> Result<Option<DoneFile>> {
        let done = read_done(&self.record.done)?;
        if let Some(record) = done.as_ref() {
            ensure!(
                record.command_id == self.id(),
                "shell reported completion for a different command"
            );
        }
        Ok(done)
    }
}
