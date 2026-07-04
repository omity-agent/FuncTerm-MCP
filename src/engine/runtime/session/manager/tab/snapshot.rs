use super::super::shell_session::ShellSession;
use crate::runtime::protocol::{ShellView, ViewResult};
use anyhow::Result;
#[derive(Clone)]
pub(in crate::engine::runtime::session::manager) struct TabSnapshot {
    pub(super) screen: String,
    cwd: String,
    title: String,
    shell_type: crate::shell::ShellChoice,
    idle: bool,
}
impl TabSnapshot {
    pub(super) fn from_session(session: &ShellSession) -> Result<Self> {
        let cwd = crate::text::path_text(&session.cwd()?, "cwd")?;
        let current_screen = session.screen_contents()?;
        let screen = if current_screen.is_empty() {
            cwd.clone()
        } else {
            current_screen
        };
        let shell_type = session.current_choice()?;
        Ok(Self {
            screen,
            cwd,
            title: session.screen_title()?,
            shell_type,
            idle: session.busy_command_id()?.is_none(),
        })
    }
    pub(in crate::engine::runtime::session::manager) fn into_view(self, alive: bool) -> ViewResult {
        let Self {
            screen,
            cwd,
            title,
            shell_type,
            idle,
        } = self;
        let shell = ShellView {
            alive,
            title,
            shell_type,
            cwd,
            idle,
        };
        ViewResult::Tab {
            shell,
            screen,
            note: String::new(),
        }
    }
    pub(in crate::engine::runtime::session::manager) fn shell_view(self, alive: bool) -> ShellView {
        ShellView {
            alive,
            title: self.title,
            shell_type: self.shell_type,
            cwd: self.cwd,
            idle: self.idle,
        }
    }
}
