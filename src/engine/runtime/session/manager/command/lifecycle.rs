use super::outcome::CommandInputHistory;
use crate::runtime::protocol::CommandSnapshot;
use crate::runtime::session::keyboard::InputDelivery;
use crate::runtime::session::observation::PathWatch;
use crate::runtime::session::records::CommandRecord;
use crate::runtime::session::terminal::CommandTitle;
use alloc::sync::Arc;
use anyhow::Result;
use core::time::Duration;
use parking_lot::Mutex;
use std::time::Instant;
pub(in crate::engine::runtime::session::manager) struct ManagedCommand {
    id: String,
    pub(super) record: CommandRecord,
    started_at: Instant,
    pub(super) state: Mutex<ManagedCommandState>,
    pub(super) title: Arc<CommandTitle>,
    watch: Arc<PathWatch>,
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
        title: Arc<CommandTitle>,
        watch: Arc<PathWatch>,
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
            title,
            watch,
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
        let result = self
            .watch
            .wait(limit, || {
                Ok(!matches!(self.state.lock().wait, CommandWait::Running)
                    || self.record.done.try_exists()?)
            })
            .await;
        let current = self.state.lock().wait;
        if !matches!(current, CommandWait::Running) {
            return Ok(current);
        }
        match result {
            Ok(true) => Ok(CommandWait::Finished),
            Ok(false) => Ok(CommandWait::Running),
            Err(error) => Err(error),
        }
    }
    pub(in crate::engine::runtime::session::manager) fn cancel_title_capture(&self) {
        drop(self.title.cancel());
    }
    pub(in crate::engine::runtime::session::manager) async fn wait_started(
        &self,
        limit: Duration,
    ) -> Result<bool> {
        self.watch
            .wait(limit, || {
                Ok(!matches!(self.state.lock().wait, CommandWait::Running)
                    || self.record.started.try_exists()?
                    || self.record.done.try_exists()?)
            })
            .await
    }
    pub(super) fn wake(&self) {
        self.watch.wake();
    }
    pub(super) fn time_consumption(&self) -> Duration {
        self.started_at.elapsed()
    }
}
