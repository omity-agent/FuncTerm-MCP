use super::Tab;
use crate::runtime::protocol::ViewResult;
use anyhow::Result;
use std::thread;
impl Tab {
    pub(super) fn view(&self, waiting: core::time::Duration) -> Result<ViewResult> {
        let Ok(session) = self.live_session() else {
            return Ok(self.snapshot_view());
        };
        let busy_command_id = session.busy_command_id();
        let Some(command_id) = busy_command_id else {
            thread::sleep(waiting);
            return self.tab_view(&session);
        };
        if let Some(command) = self.find_command(&command_id) {
            match command.wait(waiting)? {
                super::super::command::CommandWait::Finished => {
                    self.finish_done_command(&command)?;
                }
                super::super::command::CommandWait::Running => {
                    self.abort_if_shell_dead(&session, &command)?;
                }
                super::super::command::CommandWait::Failed => {}
            }
        }
        self.tab_view(&session)
    }
    fn tab_view(&self, session: &super::super::shell_session::ShellSession) -> Result<ViewResult> {
        let alive = session.is_alive()?;
        session.refresh_choice()?;
        if alive {
            let snapshot = self.remember(session)?;
            Ok(snapshot.into_view(true))
        } else {
            if let Some(command_id) = session.busy_command_id()
                && let Some(command) = self.find_command(&command_id)
            {
                command.mark_failed("shell exited before command wrote done.json")?;
                session.release(command.id());
            }
            self.close_session(session)?;
            Ok(self.snapshot_view())
        }
    }
}
