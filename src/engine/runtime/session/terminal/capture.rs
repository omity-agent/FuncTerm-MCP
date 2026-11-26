use super::{rendering::CommandScreen, transcript::RenderedOutput};
use crate::runtime::session::observation::wait_until;
use alloc::sync::Arc;
use anyhow::{Result, bail, ensure};
use bytestring::ByteString;
use core::time::Duration;
use event_listener::Event;
use parking_lot::Mutex;
use std::time::Instant;
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
#[derive(Clone)]
enum CapturePhase {
    Pending,
    Active(Instant),
    Finished(Option<Duration>),
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
    pub(in crate::engine::runtime::session) fn title(&self) -> String {
        self.state.lock().title.clone()
    }
    pub(in crate::engine::runtime::session) fn active(&self) -> bool {
        matches!(self.state.lock().phase, CapturePhase::Active(_))
    }
    pub(in crate::engine::runtime::session) fn elapsed(&self) -> Option<Duration> {
        self.state.lock().phase.elapsed()
    }
    pub(in crate::engine::runtime::session) fn finished(&self) -> Result<bool> {
        let phase = self.state.lock().phase.clone();
        match phase {
            CapturePhase::Finished(_) => Ok(true),
            CapturePhase::Pending | CapturePhase::Active(_) => Ok(false),
            CapturePhase::Failed(message) => bail!("{message}"),
        }
    }
    pub(in crate::engine::runtime::session) fn output(&self) -> Result<Option<ByteString>> {
        self.state
            .lock()
            .output
            .as_ref()
            .map(RenderedOutput::contents)
            .transpose()
    }
    pub(in crate::engine::runtime::session) async fn wait_finished(&self) -> Result<()> {
        wait_until(&self.changed, Duration::MAX, || self.finished()).await?;
        Ok(())
    }
    pub(in crate::engine::runtime::session) async fn wait_started(
        &self,
        limit: Duration,
    ) -> Result<bool> {
        wait_until(&self.changed, limit, || {
            let phase = self.state.lock().phase.clone();
            match phase {
                CapturePhase::Pending => Ok(false),
                CapturePhase::Active(_) | CapturePhase::Finished(_) => Ok(true),
                CapturePhase::Failed(message) => bail!("{message}"),
            }
        })
        .await
    }
    pub(in crate::engine::runtime::session) fn cancel(&self) -> Result<String> {
        let mut state = self.state.lock();
        if let Some(output) = state.output.as_mut() {
            output.finish()?;
        }
        state.phase = CapturePhase::Finished(state.phase.elapsed());
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
        ensure!(
            matches!(state.phase, CapturePhase::Pending),
            "terminal capture is not pending"
        );
        if let Some(output) = state.output.as_mut() {
            output.start()?;
        }
        state.phase = CapturePhase::Active(Instant::now());
        drop(state);
        self.changed.notify(usize::MAX);
        Ok(())
    }
    pub(super) fn update_title(&self, title: &str) -> bool {
        let mut state = self.state.lock();
        if !matches!(state.phase, CapturePhase::Active(_)) {
            return false;
        }
        title.clone_into(&mut state.title);
        true
    }
    pub(super) fn observe_prompt(&self, cwd: &str) {
        let mut state = self.state.lock();
        if !matches!(state.phase, CapturePhase::Active(_)) {
            return;
        }
        state.prompt_cwd = Some(cwd.to_owned());
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
                || !matches!(state.phase, CapturePhase::Pending | CapturePhase::Active(_)))
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
        if !matches!(state.phase, CapturePhase::Finished(_)) {
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
impl CapturePhase {
    fn elapsed(&self) -> Option<Duration> {
        match *self {
            Self::Active(started) => Some(started.elapsed()),
            Self::Finished(elapsed) => elapsed,
            Self::Pending | Self::Failed(_) => None,
        }
    }
}
