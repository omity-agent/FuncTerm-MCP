use super::sequence::ProtocolEvent;
use alloc::{collections::BTreeMap, sync::Arc};
use anyhow::{Result, bail};
use parking_lot::{Condvar, Mutex};
pub(in crate::engine::runtime::session) struct CommandTitle {
    state: Mutex<CommandTitleState>,
    changed: Condvar,
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
            changed: Condvar::new(),
        }
    }
    pub(in crate::engine::runtime::session) fn current(&self) -> Result<String> {
        self.state.lock().result()
    }
    pub(in crate::engine::runtime::session) fn wait_finished(&self) -> Result<String> {
        let mut state = self.state.lock();
        while matches!(state.phase, TitlePhase::Pending | TitlePhase::Active) {
            self.changed.wait(&mut state);
        }
        state.result()
    }
    pub(in crate::engine::runtime::session) fn cancel(&self) -> Result<String> {
        let mut state = self.state.lock();
        if !matches!(state.phase, TitlePhase::Failed(_)) {
            state.phase = TitlePhase::Finished;
            self.changed.notify_all();
        }
        state.result()
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
    fn finish(&self) -> Result<()> {
        drop(self.cancel()?);
        Ok(())
    }
    fn fail(&self, message: &str) {
        let mut state = self.state.lock();
        if state.phase != TitlePhase::Finished {
            state.phase = TitlePhase::Failed(message.to_owned());
            self.changed.notify_all();
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
            ProtocolEvent::Start(id) => self.start(&id),
            ProtocolEvent::End(id) => self.finish(&id),
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
        self.require(id)?.finish()?;
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
