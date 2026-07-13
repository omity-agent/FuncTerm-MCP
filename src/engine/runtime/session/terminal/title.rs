use super::sequence::ProtocolEvent;
use alloc::{collections::BTreeMap, sync::Arc};
use anyhow::{Context as _, Result, bail};
use std::sync::{Condvar, Mutex};
pub(in crate::engine::runtime::session) struct CommandTitle {
    state: Mutex<CommandTitleState>,
    changed: Condvar,
}
struct CommandTitleState {
    phase: TitlePhase,
    title: String,
    failure: Option<String>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum TitlePhase {
    Pending,
    Active,
    Finished,
    Failed,
}
impl CommandTitle {
    const fn new(initial: String) -> Self {
        Self {
            state: Mutex::new(CommandTitleState {
                phase: TitlePhase::Pending,
                title: initial,
                failure: None,
            }),
            changed: Condvar::new(),
        }
    }
    pub(in crate::engine::runtime::session) fn current(&self) -> Result<String> {
        let state = self.lock()?;
        let result = state.result();
        drop(state);
        result
    }
    pub(in crate::engine::runtime::session) fn wait_finished(&self) -> Result<String> {
        let mut state = self.lock()?;
        while matches!(state.phase, TitlePhase::Pending | TitlePhase::Active) {
            state = self.changed.wait(state).map_err(|error| {
                anyhow::anyhow!("command title mutex poisoned while waiting: {error}")
            })?;
        }
        let result = state.result();
        drop(state);
        result
    }
    pub(in crate::engine::runtime::session) fn cancel(&self) -> Result<String> {
        let mut state = self.lock()?;
        if state.phase != TitlePhase::Failed {
            state.phase = TitlePhase::Finished;
            self.changed.notify_all();
        }
        let result = state.result();
        drop(state);
        result
    }
    fn start(&self) -> Result<()> {
        let mut state = self.lock()?;
        if state.phase == TitlePhase::Pending {
            state.phase = TitlePhase::Active;
        }
        drop(state);
        Ok(())
    }
    fn update(&self, title: &str) -> Result<()> {
        let mut state = self.lock()?;
        if state.phase == TitlePhase::Active {
            title.clone_into(&mut state.title);
        }
        drop(state);
        Ok(())
    }
    fn finish(&self) -> Result<()> {
        drop(self.cancel()?);
        Ok(())
    }
    fn fail(&self, message: &str) -> Result<()> {
        let mut state = self.lock()?;
        if state.phase != TitlePhase::Finished {
            state.phase = TitlePhase::Failed;
            state.failure = Some(message.to_owned());
            self.changed.notify_all();
        }
        drop(state);
        Ok(())
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, CommandTitleState>> {
        self.state
            .lock()
            .map_err(|error| anyhow::anyhow!("command title mutex poisoned: {error}"))
    }
}
impl CommandTitleState {
    fn result(&self) -> Result<String> {
        if self.phase == TitlePhase::Failed {
            let message = self
                .failure
                .clone()
                .context("failed command title capture is missing an error")?;
            bail!(message);
        }
        Ok(self.title.clone())
    }
}
pub(super) struct CaptureRegistry {
    initial: String,
    captures: BTreeMap<String, Arc<CommandTitle>>,
    active: Option<String>,
}
impl CaptureRegistry {
    pub(super) const fn new(initial: String) -> Self {
        Self {
            initial,
            captures: BTreeMap::new(),
            active: None,
        }
    }
    pub(super) fn register(&mut self, id: &str) -> Result<Arc<CommandTitle>> {
        if self.captures.contains_key(id) {
            bail!("command title capture already exists for {id}");
        }
        let capture = Arc::new(CommandTitle::new(self.initial.clone()));
        self.captures.insert(id.to_owned(), Arc::clone(&capture));
        Ok(capture)
    }
    pub(super) fn handle(&mut self, event: ProtocolEvent, screen_title: &str) -> Result<()> {
        match event {
            ProtocolEvent::Start(id) => self.start(&id),
            ProtocolEvent::End(id) => self.finish(&id),
            ProtocolEvent::WindowTitleChanged => self.update(screen_title),
            ProtocolEvent::Invalid(message) => bail!(message),
        }
    }
    pub(super) fn fail_all(&mut self, message: &str) {
        for capture in self.captures.values() {
            if let Err(error) = capture.fail(message) {
                eprintln!("failed to report terminal reader failure: {error:#}");
            }
        }
        self.captures.clear();
        self.active = None;
    }
    fn start(&mut self, id: &str) -> Result<()> {
        if let Some(active) = self.active.as_deref() {
            bail!("command title capture {id} started while {active} is active");
        }
        self.require(id)?.start()?;
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
            self.require(id)?.update(title)?;
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
