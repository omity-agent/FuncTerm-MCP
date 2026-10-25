use super::outcome::CommandInputHistory;
use crate::runtime::protocol::{CommandSnapshot, OutputTokenizer};
use crate::runtime::session::keyboard::InputDelivery;
use crate::runtime::session::observation::PathWatch;
use crate::runtime::session::records::CommandRecord;
use crate::runtime::session::terminal::CommandCapture;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
use parking_lot::Mutex;
use std::time::Instant;
pub(in crate::engine::runtime::session::manager) struct ManagedCommand {
    id: String,
    pub(super) record: CommandRecord,
    started_at: Instant,
    pub(super) state: Mutex<ManagedCommandState>,
    pub(super) capture: Arc<CommandCapture>,
    watch: Arc<PathWatch>,
    pub(super) tokenizer: Arc<OutputTokenizer>,
    pub(super) completion: tokio::sync::Mutex<()>,
}
pub(super) struct ManagedCommandState {
    pub(super) wait: CommandWait,
    pub(super) cached_view: Option<CommandSnapshot>,
    pub(super) input: CommandInputHistory,
}
#[derive(Clone, Copy)]
pub(in crate::engine::runtime::session::manager) enum CommandWait {
    Running,
    Finished,
    Failed,
}
#[derive(Debug, thiserror :: Error)]
pub(in crate::engine::runtime::session::manager) enum CommandInputFailure {
    #[error("manual_write target command ended before input could be written")]
    CommandEnded,
    #[error(transparent)]
    Write(#[from] anyhow::Error),
}
impl ManagedCommand {
    pub(super) fn new(
        id: String,
        record: CommandRecord,
        capture: Arc<CommandCapture>,
        watch: Arc<PathWatch>,
        tokenizer: Arc<OutputTokenizer>,
    ) -> Self {
        tracing :: debug ! (command_id = % id , directory = % record . directory . display () , "command registered");
        Self {
            id,
            record,
            started_at: Instant::now(),
            state: Mutex::new(ManagedCommandState {
                wait: CommandWait::Running,
                cached_view: None,
                input: CommandInputHistory::default(),
            }),
            capture,
            watch,
            tokenizer,
            completion: tokio::sync::Mutex::new(()),
        }
    }
    pub(in crate::engine::runtime::session::manager) fn id(&self) -> &str {
        &self.id
    }
    pub(super) const fn record(&self) -> &CommandRecord {
        &self.record
    }
    pub(in crate::engine::runtime::session::manager) fn deliver_input(
        &self,
        write: impl FnOnce() -> Result<InputDelivery>,
    ) -> Result<(), CommandInputFailure> {
        let mut state = self.state.lock();
        if !matches!(state.wait, CommandWait::Running) {
            return Err(CommandInputFailure::CommandEnded);
        }
        let delivery = write()?;
        state.input.observe(delivery);
        drop(state);
        Ok(())
    }
    pub(in crate::engine::runtime::session::manager) async fn wait(
        &self,
        limit: Duration,
    ) -> Result<CommandWait> {
        let result = self.capture.wait_ended(limit).await;
        let current = self.state.lock().wait;
        if !matches!(current, CommandWait::Running) {
            return Ok(current);
        }
        match result {
            Ok(true) => Ok(CommandWait::Finished),
            Ok(false) if self.record.done.try_exists()? => Ok(CommandWait::Finished),
            Ok(false) => Ok(CommandWait::Running),
            Err(error) => Err(error),
        }
    }
    pub(in crate::engine::runtime::session::manager) fn freeze_output(&self) -> Result<()> {
        self.capture.freeze()
    }
    pub(in crate::engine::runtime::session::manager) async fn wait_started(
        &self,
        limit: Duration,
    ) -> Result<bool> {
        self.capture.wait_started(limit).await
    }
    pub(super) fn wake(&self) {
        self.watch.wake();
    }
    pub(super) async fn wait_released(&self) -> Result<()> {
        self.watch
            .wait(Duration::MAX, || {
                self.record.released.try_exists().with_context(|| {
                    format!(
                        "failed to inspect command release {}",
                        self.record.released.display()
                    )
                })
            })
            .await?;
        Ok(())
    }
    pub(super) fn time_consumption(&self) -> Duration {
        self.started_at.elapsed()
    }
}
