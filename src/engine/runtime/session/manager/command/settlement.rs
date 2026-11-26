use super::super::{shell_session::ShellSession, tab::Tab};
use super::{ManagedCommand, completion::Completion};
use crate::contract::CommandReport;
use crate::runtime::session::observation::PathEvents;
use crate::runtime::session::terminal::{ShellPrompt, Terminal};
use crate::shell::frame::ShellRoute;
use alloc::sync::Arc;
use anyhow::Result;
use core::time::Duration;
use std::path::Path;
#[derive(Clone, Copy)]
pub(in crate::engine::runtime::session::manager) enum SessionEnd {
    Closed,
    Exited(i32),
    Failed,
}
pub(super) async fn wait_for_resumption(
    command: &ManagedCommand,
    terminal: &Terminal,
    events: &PathEvents,
    tab_root: &Path,
) -> Result<Option<Arc<ShellPrompt>>> {
    let published = events.wait(Duration::MAX, || {
        let route = ShellRoute::read(tab_root)?;
        Ok(route.frame != command.frame && route.continuation.is_none())
    });
    tokio::select! {
        biased;
        ready = published => {
            ready?;
            Ok(None)
        }
        prompted = parent_prompt(command, terminal, tab_root) => prompted.map(Some),
    }
}
async fn parent_prompt(
    command: &ManagedCommand,
    terminal: &Terminal,
    tab_root: &Path,
) -> Result<Arc<ShellPrompt>> {
    let mut after = command.registered_at();
    loop {
        let prompt = terminal.wait_for_prompt_after(after).await?;
        after = prompt.observed_at;
        let route = ShellRoute::read(tab_root)?;
        if route.frame != command.frame
            && route.frame.id() == prompt.frame_id
            && route.continuation.is_some()
        {
            return Ok(prompt);
        }
    }
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
        settled
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
        mut report: CommandReport<String>,
    ) -> Result<()> {
        session.refresh_route()?;
        let exits_shell = report.outcome.exits_shell();
        if exits_shell && !command.frame.is_root() {
            report.cwd =
                crate::text::path_text(&session.handoff_cwd(), "restored shell working directory")?;
        }
        command.settle(Completion::Reported(&report), |cwd| session.set_cwd(cwd))?;
        session.release(command.id());
        if exits_shell && command.frame.is_root() {
            self.end_session(session, SessionEnd::Closed)?;
        }
        Ok(())
    }
}
