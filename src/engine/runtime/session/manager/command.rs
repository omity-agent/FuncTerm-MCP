mod lifecycle;
mod result_view;
mod settlement;
mod start;
mod supervision;
use super::Manager;
use crate::runtime::protocol::{KeyboardInput, ViewResult};
use alloc::sync::Arc;
use anyhow::Result;
use core::num::NonZeroUsize;
use core::time::Duration;
pub(super) use lifecycle::ManagedCommand;
pub(super) use settlement::SessionEnd;
impl Manager {
    pub(crate) async fn manual_write(
        &self,
        tab_id: &str,
        input: KeyboardInput,
        wait_timeout: Duration,
    ) -> Result<ViewResult> {
        self.tabs.manual_write(tab_id, input, wait_timeout).await
    }
    pub(crate) async fn run(
        self: &Arc<Self>,
        tab_id: &str,
        command: &str,
        separate_output: bool,
        wait_timeout: Duration,
    ) -> Result<(String, ViewResult)> {
        self.tabs
            .run(tab_id, command, separate_output, wait_timeout)
            .await
    }
    pub(crate) async fn view(
        &self,
        id: &str,
        wait_timeout: Duration,
        start_token: NonZeroUsize,
    ) -> Result<ViewResult> {
        let view = self.tabs.view(id, wait_timeout).await?;
        if start_token == NonZeroUsize::MIN {
            return Ok(view);
        }
        let tokenizer = Arc::clone(&self.tokenizer);
        self.blocking
            .run(move || tokenizer.start_at(view, start_token))
            .await
    }
}
mod completion;
mod delivery;
