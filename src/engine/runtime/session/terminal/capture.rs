use super::rendering::{CommandScreen, RenderedOutput};
use crate::runtime::session::observation::wait_until;
use alloc::sync::Arc;
use anyhow::{Result, bail};
use bytestring::ByteString;
use core::time::Duration;
use event_listener::Event;
use parking_lot::Mutex;
pub(in crate::engine::runtime::session) struct CommandCapture {
    state: Mutex<CaptureState>,
    changed: Event,
}
struct CaptureState {
    phase: CapturePhase,
    title: String,
    output: Option<RenderedOutput>,
    prompt_cwd: Option<String>,
    prompt_revision: u64,
}
#[derive(Clone, PartialEq, Eq)]
enum CapturePhase {
    Pending,
    Active,
    Finished,
    Failed(String),
}
impl CommandCapture {
    pub(super) fn new(
        initial: String,
        separate_output: bool,
        screen: Arc<Mutex<CommandScreen>>,
    ) -> Self {
        Self {
            state: Mutex::new(CaptureState {
                phase: CapturePhase::Pending,
                title: initial,
                output: (!separate_output).then(|| RenderedOutput::new(screen)),
                prompt_cwd: None,
                prompt_revision: 0,
            }),
            changed: Event::new(),
        }
    }
    pub(in crate::engine::runtime::session) fn current(&self) -> Result<String> {
        let state = self.state.lock();
        if let CapturePhase::Failed(message) = state.phase.clone() {
            bail!("{message}");
        }
        Ok(state.title.clone())
    }
    pub(in crate::engine::runtime::session) fn output(&self) -> Result<Option<ByteString>> {
        self.state
            .lock()
            .output
            .as_ref()
            .map(RenderedOutput::contents)
            .transpose()
    }
    pub(in crate::engine::runtime::session) async fn wait_finished(&self) -> Result<String> {
        self.wait_ended(Duration::MAX).await?;
        self.current()
    }
    pub(in crate::engine::runtime::session) async fn wait_ended(
        &self,
        limit: Duration,
    ) -> Result<bool> {
        let ended = wait_until(&self.changed, limit, || {
            Ok(!matches!(
                self.state.lock().phase,
                CapturePhase::Pending | CapturePhase::Active
            ))
        })
        .await?;
        Ok(ended && self.state.lock().phase == CapturePhase::Finished)
    }
    pub(in crate::engine::runtime::session) async fn wait_started(
        &self,
        limit: Duration,
    ) -> Result<bool> {
        wait_until(&self.changed, limit, || {
            Ok(self.state.lock().phase != CapturePhase::Pending)
        })
        .await
    }
    pub(in crate::engine::runtime::session) fn cancel(&self) -> Result<String> {
        let mut state = self.state.lock();
        if let Some(output) = state.output.as_mut() {
            output.finish()?;
        }
        state.phase = CapturePhase::Finished;
        self.changed.notify(usize::MAX);
        Ok(state.title.clone())
    }
    pub(in crate::engine::runtime::session) fn freeze(&self) -> Result<()> {
        if let Some(output) = self.state.lock().output.as_mut() {
            output.finish()?;
        }
        Ok(())
    }
    pub(super) fn start(&self) -> Result<()> {
        let mut state = self.state.lock();
        if state.phase == CapturePhase::Pending {
            if let Some(output) = state.output.as_mut() {
                output.start()?;
            }
            state.phase = CapturePhase::Active;
        }
        drop(state);
        self.changed.notify(usize::MAX);
        Ok(())
    }
    pub(super) fn update(&self, title: &str) {
        let mut state = self.state.lock();
        if state.phase == CapturePhase::Active {
            title.clone_into(&mut state.title);
        }
    }
    pub(super) fn observe_prompt(&self, cwd: String) {
        let mut state = self.state.lock();
        state.prompt_cwd = Some(cwd);
        state.prompt_revision = state
            .prompt_revision
            .checked_add(1)
            .unwrap_or_else(|| panic!("terminal prompt revision overflowed"));
        drop(state);
        self.changed.notify(usize::MAX);
    }
    pub(in crate::engine::runtime::session) async fn wait_prompt(
        &self,
        after: u64,
    ) -> Result<Option<(u64, String)>> {
        wait_until(&self.changed, Duration::MAX, || {
            let state = self.state.lock();
            Ok(state.prompt_revision > after
                || !matches!(state.phase, CapturePhase::Pending | CapturePhase::Active))
        })
        .await?;
        let state = self.state.lock();
        Ok(state
            .prompt_cwd
            .as_ref()
            .filter(|_| state.prompt_revision > after)
            .map(|cwd| (state.prompt_revision, cwd.clone())))
    }
    pub(super) fn fail(&self, message: &str) {
        let mut state = self.state.lock();
        if state.phase != CapturePhase::Finished {
            let failure = if let Some(output) = state.output.as_mut()
                && let Err(error) = output.finish()
            {
                format!("{message}; failed to freeze terminal output: {error:#}")
            } else {
                message.to_owned()
            };
            state.phase = CapturePhase::Failed(failure);
            self.changed.notify(usize::MAX);
        }
        drop(state);
    }
}
