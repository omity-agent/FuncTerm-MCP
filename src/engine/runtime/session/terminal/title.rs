use super::output_events::ProtocolEvent;
use crate::runtime::session::observation::wait_until;
use alloc::{collections::BTreeMap, sync::Arc};
use anyhow::{Result, bail};
use core::time::Duration;
use event_listener::Event;
use parking_lot::Mutex;
pub(in crate::engine::runtime::session) struct CommandTitle {
    state: Mutex<CommandTitleState>,
    changed: Event,
}
struct CommandTitleState {
    phase: TitlePhase,
    title: String,
}
#[derive(Clone, PartialEq, Eq)]
enum TitlePhase {
    Pending,
    Active,
    Finished,
    Failed(String),
}
impl CommandTitle {
    const fn new(initial: String) -> Self {
        Self {
            state: Mutex::new(CommandTitleState {
                phase: TitlePhase::Pending,
                title: initial,
            }),
            changed: Event::new(),
        }
    }
    pub(in crate::engine::runtime::session) fn current(&self) -> Result<String> {
        self.state.lock().result()
    }
    pub(in crate::engine::runtime::session) async fn wait_finished(&self) -> Result<String> {
        wait_until(&self.changed, Duration::MAX, || {
            Ok(!matches!(
                self.state.lock().phase,
                TitlePhase::Pending | TitlePhase::Active
            ))
        })
        .await?;
        self.current()
    }
    pub(in crate::engine::runtime::session) fn cancel(&self) -> String {
        let mut state = self.state.lock();
        state.phase = TitlePhase::Finished;
        self.changed.notify(usize::MAX);
        state.title.clone()
    }
    fn start(&self) {
        let mut state = self.state.lock();
        if state.phase == TitlePhase::Pending {
            state.phase = TitlePhase::Active;
        }
    }
    fn update(&self, title: &str) {
        let mut state = self.state.lock();
        if state.phase == TitlePhase::Active {
            title.clone_into(&mut state.title);
        }
    }
    fn fail(&self, message: &str) {
        let mut state = self.state.lock();
        if state.phase != TitlePhase::Finished {
            state.phase = TitlePhase::Failed(message.to_owned());
            self.changed.notify(usize::MAX);
        }
        drop(state);
    }
}
impl CommandTitleState {
    fn result(&self) -> Result<String> {
        if let TitlePhase::Failed(message) = self.phase.clone() {
            bail!(message);
        }
        Ok(self.title.clone())
    }
}
pub(super) struct CaptureRegistry {
    model_title: String,
    captures: BTreeMap<String, Arc<CommandTitle>>,
    active: Option<String>,
}
impl CaptureRegistry {
    pub(super) const fn new(model_title: String) -> Self {
        Self {
            model_title,
            captures: BTreeMap::new(),
            active: None,
        }
    }
    pub(super) fn register(&mut self, id: &str) -> Result<Arc<CommandTitle>> {
        if self.captures.contains_key(id) {
            bail!("command title capture already exists for {id}");
        }
        let capture = Arc::new(CommandTitle::new(self.model_title.clone()));
        self.captures.insert(id.to_owned(), Arc::clone(&capture));
        Ok(capture)
    }
    pub(super) fn handle(&mut self, event: ProtocolEvent, screen_title: &str) -> Result<()> {
        match event {
            ProtocolEvent::Start(id) => {
                tracing :: debug ! (command_id = % id , "terminal command start marker");
                self.start(&id)
            }
            ProtocolEvent::End(id) => {
                tracing :: debug ! (command_id = % id , "terminal command end marker");
                self.finish(&id)
            }
            ProtocolEvent::WindowTitleAssigned => self.update(screen_title),
            ProtocolEvent::Invalid(message) => bail!(message),
        }
    }
    pub(super) fn fail_all(&mut self, message: &str) {
        for capture in self.captures.values() {
            capture.fail(message);
        }
        self.captures.clear();
        self.active = None;
    }
    fn start(&mut self, id: &str) -> Result<()> {
        if let Some(active) = self.active.as_deref() {
            bail!("command title capture {id} started while {active} is active");
        }
        self.require(id)?.start();
        self.active = Some(id.to_owned());
        Ok(())
    }
    fn finish(&mut self, id: &str) -> Result<()> {
        if let Some(active) = self.active.as_deref()
            && active != id
        {
            bail!("command title capture {id} ended while {active} is active");
        }
        drop(self.require(id)?.cancel());
        self.captures.remove(id);
        if self.active.as_deref() == Some(id) {
            self.active = None;
        }
        Ok(())
    }
    fn update(&self, title: &str) -> Result<()> {
        if let Some(id) = self.active.as_deref() {
            self.require(id)?.update(title);
        }
        Ok(())
    }
    fn require(&self, id: &str) -> Result<Arc<CommandTitle>> {
        self.captures
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("terminal marker references unknown command {id}"))
    }
}
