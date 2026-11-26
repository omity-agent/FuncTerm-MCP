use super::ShellSession;
use crate::contract::{CommandOutcome, CommandReport, command_directory};
use crate::runtime::protocol::RequestRejected;
use crate::runtime::session::terminal::ShellPrompt;
use crate::shell::ShellChoice;
use crate::shell::frame::{ShellFrame, ShellRoute};
use anyhow::{Context as _, Result, ensure};
use std::path::PathBuf;

impl ShellSession {
    pub(in crate::engine::runtime::session::manager) fn refresh_route(&self) -> Result<()> {
        let next = ShellRoute::read(&self.tab_root)?;
        let mut route = self.route.lock();
        if next.continuation.is_none()
            && (route.frame != next.frame || route.continuation.is_some())
        {
            self.set_cwd(next.cwd.clone());
        }
        *route = next;
        drop(route);
        Ok(())
    }
    pub(in crate::engine::runtime::session::manager) fn current_choice(&self) -> ShellChoice {
        self.route.lock().frame.choice
    }
    pub(in crate::engine::runtime::session::manager) fn dispatch_frame(
        &self,
        tab_id: &str,
    ) -> Result<ShellFrame> {
        let route = self.route.lock();
        if let Some(id) = route.continuation.as_deref() {
            return Err(RequestRejected::new(format!(
                "The command was not executed because `{tab_id}` is busy with `{id}`"
            ))
            .into());
        }
        Ok(route.frame)
    }
    pub(in crate::engine::runtime::session::manager) fn resuming(&self) -> bool {
        self.route.lock().continuation.is_some()
    }
    pub(in crate::engine::runtime::session::manager) fn handoff_cwd(&self) -> PathBuf {
        self.route.lock().cwd.clone()
    }
    pub(in crate::engine::runtime::session::manager) fn resume_at_prompt(
        &self,
        prompt: &ShellPrompt,
    ) -> Result<()> {
        let route = ShellRoute::read(&self.tab_root)?;
        let Some(id) = route.continuation.as_deref() else {
            return Ok(());
        };
        ensure!(
            route.frame.id() == prompt.frame_id,
            "shell changed while resuming its prompt"
        );
        let directory = command_directory(&self.command_root, id)?;
        let report =
            CommandReport::read(&directory)?.context("missing suspended command report")?;
        ensure!(
            report.command_id == id && matches!(report.outcome, CommandOutcome::HandedOff),
            "unexpected report for suspended command"
        );
        crate::app::command_state::release_handoff(&directory, id, &prompt.cwd, true)
    }
}
