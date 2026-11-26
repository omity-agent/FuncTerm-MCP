mod buffering;
mod history;
use self::{buffering::SeparatedOutput, history::CommandInputHistory};
use crate::runtime::protocol::{
    CommandOutput, CommandSnapshot, EndReason, OutputTokenizer, OutputTokens, RequestRejected,
};
use crate::runtime::session::keyboard::InputDelivery;
use crate::runtime::session::observation::{PathEvents, wait_until};
use crate::runtime::session::records::{CommandRecord, DoneFile, read_command_result};
use crate::runtime::session::terminal::CommandCapture;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, ensure};
use core::time::Duration;
use event_listener::Event;
use parking_lot::Mutex;
use std::time::Instant;
pub(in crate::engine::runtime::session::manager) struct ManagedCommand {
    id: String,
    pub(super) record: CommandRecord,
    started_at: Instant,
    start_timeout: Duration,
    pub(super) state: Mutex<ManagedCommandState>,
    pub(super) capture: Arc<CommandCapture>,
    pub(super) tokenizer: Arc<OutputTokenizer>,
    pub(super) settled: Event,
}
pub(super) struct ManagedCommandState {
    pub(super) outcome: Option<Result<SettledCommand, String>>,
    pub(super) input: CommandInputHistory,
    separated: Option<SeparatedOutput>,
    terminal_tokens: OutputTokens,
}
pub(super) struct SettledCommand {
    pub(super) reason: EndReason,
    pub(super) view: CommandSnapshot,
}
impl ManagedCommandState {
    pub(super) fn finish(&mut self, outcome: Result<SettledCommand, String>) {
        self.outcome = Some(outcome);
        self.separated = None;
        self.terminal_tokens = OutputTokens::default();
    }
}
impl ManagedCommand {
    pub(super) fn new(
        id: String,
        record: CommandRecord,
        capture: Arc<CommandCapture>,
        tokenizer: Arc<OutputTokenizer>,
        start_timeout: Duration,
    ) -> Self {
        tracing :: debug ! (command_id = % id , directory = % record . directory . display () , "command registered");
        Self {
            id,
            record,
            started_at: Instant::now(),
            start_timeout,
            state: Mutex::new(ManagedCommandState {
                outcome: None,
                input: CommandInputHistory::default(),
                separated: None,
                terminal_tokens: OutputTokens::default(),
            }),
            capture,
            tokenizer,
            settled: Event::new(),
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
    ) -> Result<()> {
        let mut state = self.state.lock();
        if state.outcome.is_some() || !self.capture.active() || self.record.done.try_exists()? {
            return Err(RequestRejected::new(
                "manual_write target command ended before input could be written",
            )
            .into());
        }
        let delivery = write()?;
        state.input.observe(delivery);
        drop(state);
        Ok(())
    }
    pub(in crate::engine::runtime::session::manager) async fn wait(
        &self,
        limit: Duration,
    ) -> Result<()> {
        wait_until(&self.settled, limit, || Ok(self.is_settled())).await?;
        Ok(())
    }
    pub(super) fn is_settled(&self) -> bool {
        self.state.lock().outcome.is_some()
    }
    pub(super) async fn wait_for_start(&self) -> Result<()> {
        ensure!(
            self.capture.wait_started(self.start_timeout).await?,
            "shell did not start command within {:?}",
            self.start_timeout
        );
        Ok(())
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
    pub(super) async fn wait_released(&self, events: &PathEvents) -> Result<()> {
        events
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
    pub(super) fn snapshot(
        &self,
        state: &mut ManagedCommandState,
        title: String,
        done: Option<&DoneFile>,
        finished: bool,
    ) -> Result<CommandSnapshot> {
        let (output, tokens) = if let Some(output) = self.capture.output()? {
            let count = state.terminal_tokens.count(&self.tokenizer, &output);
            let captured = CommandOutput::Terminal { output };
            (captured, count)
        } else {
            state
                .separated
                .get_or_insert_default()
                .read(&self.record, &self.tokenizer, finished)?
        };
        read_command_result(self.time_consumption(), title, output, tokens, done)
    }
}
