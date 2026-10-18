use crate::runtime::session::observation::wait_until;
use anyhow::{Result, bail};
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
    output: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
enum CapturePhase {
    Pending,
    Active,
    Finished,
    Failed(String),
}

impl CommandCapture {
    pub(super) fn new(initial: String, separate_output: bool) -> Self {
        Self {
            state: Mutex::new(CaptureState {
                phase: CapturePhase::Pending,
                title: initial,
                output: (!separate_output).then(String::new),
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

    pub(in crate::engine::runtime::session) fn output(&self) -> Option<String> {
        self.state.lock().output.clone()
    }

    pub(in crate::engine::runtime::session) async fn wait_finished(&self) -> Result<String> {
        wait_until(&self.changed, Duration::MAX, || {
            Ok(!matches!(
                self.state.lock().phase,
                CapturePhase::Pending | CapturePhase::Active
            ))
        })
        .await?;
        self.current()
    }

    pub(in crate::engine::runtime::session) fn cancel(&self) -> String {
        let mut state = self.state.lock();
        state.phase = CapturePhase::Finished;
        self.changed.notify(usize::MAX);
        state.title.clone()
    }

    pub(super) fn start(&self) {
        let mut state = self.state.lock();
        if state.phase == CapturePhase::Pending {
            state.phase = CapturePhase::Active;
        }
    }

    pub(super) fn update(&self, title: &str) {
        let mut state = self.state.lock();
        if state.phase == CapturePhase::Active {
            title.clone_into(&mut state.title);
        }
    }

    pub(super) fn record(&self, text: &str) {
        let mut state = self.state.lock();
        if state.phase == CapturePhase::Active
            && let Some(output) = state.output.as_mut()
        {
            output.push_str(text);
        }
    }

    pub(super) fn fail(&self, message: &str) {
        let mut state = self.state.lock();
        if state.phase != CapturePhase::Finished {
            state.phase = CapturePhase::Failed(message.to_owned());
            self.changed.notify(usize::MAX);
        }
        drop(state);
    }
}
