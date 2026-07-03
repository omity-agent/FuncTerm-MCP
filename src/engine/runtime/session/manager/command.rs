mod lifecycle;
mod result_view;
mod start;
#[cfg(test)]
#[path = "command/command_tests.rs"]
mod tests;
use super::Manager;
use crate::runtime::protocol::{EndReason, ViewResult};
use alloc::sync::Arc;
use anyhow::Result;
use core::time::Duration;
pub(super) use lifecycle::{CommandWait, ManagedCommand};
impl Manager {
    pub(crate) fn manual_write(&self, tab_id: &str, bytes: &[u8]) -> Result<ViewResult> {
        self.tabs.manual_write(tab_id, bytes)
    }
    pub(crate) fn send_command(
        self: &Arc<Self>,
        tab_id: &str,
        command: &str,
        waiting: Duration,
    ) -> Result<(String, EndReason, ViewResult)> {
        self.tabs.send_command(tab_id, command, waiting)
    }
    pub(crate) fn view(&self, id: &str, waiting: Duration) -> Result<ViewResult> {
        self.tabs.view(id, waiting)
    }
}
