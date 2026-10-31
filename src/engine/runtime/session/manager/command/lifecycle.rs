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
}
pub(super) struct ManagedCommandState {
    pub(super) wait: CommandWait,
    pub(super) cached_view: Option<CommandSnapshot>,
    pub(super) input: CommandInputHistory,
    pub(super) session_exit: bool,
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
                session_exit: false,
            }),
            capture,
            watch,
            tokenizer,
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
    pub(in crate::engine::runtime::session::manager) async fn wait_started(
        &self,
        limit: Duration,
    ) -> Result<bool> {
        self.capture.wait_started(limit).await
    }
    pub(super) async fn interrupted_prompt(&self) -> Result<Option<String>> {
        let mut revision = 0;
        while let Some((next, cwd)) = self.capture.wait_prompt(revision).await? {
            revision = next;
            if self.state.lock().input.interrupted() {
                return Ok(Some(cwd));
            }
        }
        Ok(None)
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
    pub(super) fn requests_session_exit(&self) -> Result<bool> {
        let state = self.state.lock();
        if !matches!(state.wait, CommandWait::Running) {
            return Ok(state.session_exit);
        }
        drop(state);
        self.record.session_exit.try_exists().with_context(|| {
            format!(
                "failed to inspect session exit marker {}",
                self.record.session_exit.display()
            )
        })
    }
}
