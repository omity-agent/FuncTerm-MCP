use super::{Manager, tabs::Tab};
use crate::runtime::protocol::ViewResult;
use crate::runtime::session::records::wait_for_done;
use anyhow::Result;
use std::thread;
impl Manager {
    pub(crate) fn view(&self, id: &str, waiting: core::time::Duration) -> Result<ViewResult> {
        self.tabs.view(id, waiting)
    }
}
impl Tab {
    pub(super) fn view(&self, waiting: core::time::Duration) -> Result<ViewResult> {
        let Ok(session) = self.live_session() else {
            return self.snapshot_view();
        };
        let busy_command_id = session.busy_command_id()?;
        let Some(command_id) = busy_command_id else {
            thread::sleep(waiting);
            return self.tab_view(&session);
        };
        if let Some(record) = self.find_command_for_view(&command_id)?
            && wait_for_done(&record.done, waiting)?
        {
            session.update_cwd_from_done(&record)?;
        }
        self.tab_view(&session)
    }
    fn tab_view(&self, session: &super::session::ShellSession) -> Result<ViewResult> {
        let alive = session.is_alive()?;
        session.refresh_choice()?;
        if alive {
            let snapshot = self.remember(session)?;
            Ok(snapshot.into_view(true))
        } else {
            self.close_session(session)?;
            self.snapshot_view()
        }
    }
}
