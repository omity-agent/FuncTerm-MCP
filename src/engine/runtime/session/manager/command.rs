mod lifecycle;
mod outcome;
mod result_view;
mod start;
#[cfg(test)]
#[path = "../../../../../tests/unit/runtime/input_lifecycle.rs"]
mod tests;
use super::Manager;
use crate::runtime::protocol::{EndReason, KeyboardInput, ViewResult};
use alloc::sync::Arc;
use anyhow::Result;
use core::time::Duration;
pub(in crate::engine::runtime::session::manager) use lifecycle::CommandInputFailure;
pub(super) use lifecycle::{CommandWait, ManagedCommand};
impl Manager {
    pub(crate) fn manual_write(
        &self,
        tab_id: &str,
        input: &KeyboardInput,
        wait_timeout: Duration,
    ) -> Result<ViewResult> {
        self.tabs.manual_write(tab_id, input, wait_timeout)
    }
    pub(crate) fn send_command(
        self: &Arc<Self>,
        tab_id: &str,
        command: &str,
        wait_timeout: Duration,
    ) -> Result<(String, EndReason, ViewResult)> {
        self.tabs.send_command(tab_id, command, wait_timeout)
    }
    pub(crate) fn view(&self, id: &str, wait_timeout: Duration) -> Result<ViewResult> {
        self.tabs.view(id, wait_timeout)
    }
}
