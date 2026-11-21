use super::{capture::CommandCapture, markers::ProtocolEvent, rendering::CommandScreen};
use alloc::{collections::BTreeMap, sync::Arc};
use anyhow::{Result, bail};
use parking_lot::Mutex;
pub(super) struct CaptureRegistry {
    title: String,
    captures: BTreeMap<String, Arc<CommandCapture>>,
    active: Option<ActiveCapture>,
}
struct ActiveCapture {
    id: String,
    capture: Arc<CommandCapture>,
    titles: TitleUpdates,
}
#[derive(Default)]
enum TitleUpdates {
    #[default]
    Live,
    AwaitingEnd,
    Deferred(String),
}
#[derive(Clone, Copy)]
pub(super) enum CaptureEnd {
    Marker,
    Prompt,
}
impl CaptureRegistry {
    pub(super) const fn new(title: String) -> Self {
        Self {
            title,
            captures: BTreeMap::new(),
            active: None,
        }
    }
    pub(super) fn register(
        &mut self,
        id: &str,
        separate_output: bool,
        screen: Arc<Mutex<CommandScreen>>,
    ) -> Result<Arc<CommandCapture>> {
        if self.captures.contains_key(id) {
            bail!("command capture already exists for {id}");
        }
        let capture = Arc::new(CommandCapture::new(
            self.title.clone(),
            separate_output,
            screen,
        ));
        self.captures.insert(id.to_owned(), Arc::clone(&capture));
        tracing::trace!(command_id = %id, captures = self.captures.len(), "terminal command capture registered");
        Ok(capture)
    }
    pub(super) fn handle(&mut self, event: ProtocolEvent) -> Result<()> {
        match event {
            ProtocolEvent::Start(id) => {
                tracing::debug!(command_id = %id, "terminal command start marker");
                self.start(&id)
            }
            ProtocolEvent::End(id) => {
                tracing::debug!(command_id = %id, "terminal command end marker");
                self.finish(&id, CaptureEnd::Marker)
            }
            ProtocolEvent::Prompt(cwd) => {
                if let Some(active) = self.active.as_ref() {
                    active.capture.observe_prompt(cwd);
                }
                Ok(())
            }
        }
    }
    pub(super) fn fail_all(&mut self, message: &str) {
        for capture in self.captures.values() {
            capture.fail(message);
        }
        self.captures.clear();
        self.active = None;
    }
    pub(super) fn close_all(&mut self) -> Result<()> {
        for capture in self.captures.values() {
            capture.freeze()?;
        }
        self.captures.clear();
        self.active = None;
        Ok(())
    }
    fn start(&mut self, id: &str) -> Result<()> {
        if let Some(active) = self.active.as_ref() {
            bail!("command capture {id} started while {} is active", active.id);
        }
        let capture = self.require(id)?;
        capture.start()?;
        self.active = Some(ActiveCapture {
            id: id.to_owned(),
            capture,
            titles: TitleUpdates::Live,
        });
        Ok(())
    }
    pub(super) fn finish(&mut self, id: &str, end: CaptureEnd) -> Result<()> {
        if let Some(active) = self.active.as_mut() {
            if active.id != id {
                bail!("command capture {id} ended while {} is active", active.id);
            }
            if matches!(end, CaptureEnd::Marker)
                && let TitleUpdates::Deferred(title) = core::mem::take(&mut active.titles)
                && active.capture.update_title(&title)
            {
                self.title = title;
            }
        }
        drop(self.require(id)?.cancel()?);
        self.captures.remove(id);
        self.active = None;
        Ok(())
    }
    pub(super) fn title(&self) -> &str {
        &self.title
    }
    pub(super) fn defer_window_titles(&mut self, id: &str) {
        if let Some(active) = self.active.as_mut()
            && active.id == id
            && matches!(active.titles, TitleUpdates::Live)
        {
            active.titles = TitleUpdates::AwaitingEnd;
        }
    }
    pub(super) fn record_window_title(&mut self, title: String) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if matches!(active.titles, TitleUpdates::Live) {
            if active.capture.update_title(&title) {
                self.title = title;
            }
        } else {
            active.titles = TitleUpdates::Deferred(title);
        }
    }
    fn require(&self, id: &str) -> Result<Arc<CommandCapture>> {
        self.captures
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("terminal marker references unknown command {id}"))
    }
}
