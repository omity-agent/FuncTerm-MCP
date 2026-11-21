use super::super::tab::Tab;
use super::ManagedCommand;
use crate::runtime::protocol::ViewResult;
use alloc::sync::Arc;
use anyhow::Result;
use core::time::Duration;
impl Tab {
    pub(in crate::engine::runtime::session::manager) async fn command_view(
        self: &Arc<Self>,
        command: &Arc<ManagedCommand>,
        wait_timeout: Duration,
    ) -> Result<ViewResult> {
        command.wait(wait_timeout).await?;
        let managed = Arc::clone(command);
        self.with_operation(move |tab| {
            let (end_reason, snapshot) = managed.view()?;
            let mut shell = if let Some(session) = tab.optional_session() {
                let alive = session.is_alive()?;
                tab.remember(&session)?.shell_view(alive)
            } else {
                tab.snapshot_shell_view()
            };
            shell.title = snapshot.title;
            Ok(ViewResult::Command {
                shell,
                command: snapshot.command,
                note: snapshot.note,
                end_reason,
            })
        })
        .await
    }
}
