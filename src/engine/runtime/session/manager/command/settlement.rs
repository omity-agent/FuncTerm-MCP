use super::super::{shell_session::ShellSession, tab::Tab};
use super::{ManagedCommand, completion::Completion};
use alloc::sync::Arc;
use anyhow::Result;
#[derive(Clone, Copy)]
pub(in crate::engine::runtime::session::manager) enum SessionEnd {
    Closed,
    Exited(i32),
    Failed,
}
impl Tab {
    pub(in crate::engine::runtime::session::manager) fn end_session(
        &self,
        session: &ShellSession,
        end: SessionEnd,
    ) -> Result<()> {
        if let Err(error) = session.terminate() {
            if let Some(command) = session.busy_command() {
                command.mark_unavailable(&error);
            }
            return Err(error);
        }
        let settlement = session.busy_command().map_or(Ok(()), |command| {
            Self::settle_closing(session, &command, end)
        });
        let recorded = self.record_closure(session);
        settlement.and(recorded)
    }
    fn settle_closing(
        session: &ShellSession,
        command: &ManagedCommand,
        end: SessionEnd,
    ) -> Result<()> {
        let completion = match end {
            SessionEnd::Closed => Completion::Closed,
            SessionEnd::Exited(code) => Completion::Exited(code),
            SessionEnd::Failed => Completion::Failed,
        };
        let settled = command.settle(completion, |cwd| session.set_cwd(cwd));
        if let Err(error) = settled.as_ref() {
            command.mark_unavailable(error);
        }
        session.release(command.id());
        settled.map(|_| ())
    }
    pub(in crate::engine::runtime::session::manager) async fn stop_after_failure(
        self: &Arc<Self>,
        command: Arc<ManagedCommand>,
    ) -> Result<()> {
        self.with_operation(move |tab| {
            if let Some(session) = tab.active_session()?
                && session.holds_command(&command)
            {
                tab.end_session(&session, SessionEnd::Failed)?;
            }
            Ok(())
        })
        .await
    }
    pub(super) fn settle_reported(
        &self,
        session: &ShellSession,
        command: &ManagedCommand,
    ) -> Result<()> {
        let requests_exit = command.settle(Completion::Reported, |cwd| session.set_cwd(cwd))?;
        session.release(command.id());
        if requests_exit {
            self.end_session(session, SessionEnd::Closed)?;
        }
        Ok(())
    }
}
