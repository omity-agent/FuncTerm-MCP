use super::Tab;
use crate::runtime::session::manager::command::SessionEnd;
use crate::runtime::session::manager::shell_session::ShellSession;
use crate::runtime::session::terminal::Terminal;
use alloc::sync::Arc;
use anyhow::{Result, bail};
impl Tab {
    pub(in crate::engine::runtime::session::manager) fn supervise_lifetime(
        self: &Arc<Self>,
        terminal: Arc<Terminal>,
    ) {
        let owner = Arc::clone(self);
        self.supervisors.spawn(async move {
            let exited = match terminal.wait_for_exit().await {
                Ok(false) => return,
                Ok(true) => true,
                Err(error) => {
                    tracing::error!(tab_id = owner.id(), "shell process observation failed");
                    tracing::debug!(
                        error = format!("{error:#}"),
                        "shell observation failure details"
                    );
                    false
                }
            };
            if let Err(error) = owner
                .with_operation(move |tab| tab.close_after_observation(exited))
                .await
            {
                tracing::error!(tab_id = owner.id(), "failed to close Tab after shell exit");
                tracing::debug!(error = format!("{error:#}"), "Tab closure failure details");
            }
        });
    }
    fn close_after_observation(&self, exited: bool) -> Result<()> {
        let Some(session) = self.optional_session() else {
            return Ok(());
        };
        let end = match session.exit_code()? {
            Some(code) => SessionEnd::Exited(code),
            None if exited => bail!("shell exit was observed without an exit status"),
            None => SessionEnd::Failed,
        };
        self.end_session(&session, end)
    }
    pub(in crate::engine::runtime::session::manager) fn active_session(
        &self,
    ) -> Result<Option<Arc<ShellSession>>> {
        let Some(session) = self.optional_session() else {
            return Ok(None);
        };
        if let Some(code) = session.exit_code()? {
            self.end_session(&session, SessionEnd::Exited(code))?;
            return Ok(None);
        }
        Ok(Some(session))
    }
    pub(in crate::engine::runtime::session::manager) fn live_session(
        &self,
    ) -> Result<Arc<ShellSession>> {
        self.active_session()?
            .ok_or_else(|| self.closed_tab().into())
    }
    pub(in crate::engine::runtime::session::manager) async fn close(
        self: &Arc<Self>,
    ) -> Result<()> {
        self.with_operation(|tab| {
            if let Some(session) = tab.optional_session() {
                tab.end_session(&session, SessionEnd::Closed)?;
            }
            Ok(())
        })
        .await
    }
}
