use super::{output_events::ProtocolEvent, transcript::CommandCapture};
use alloc::{collections::BTreeMap, sync::Arc};
use anyhow::{Result, bail};

pub(super) struct CaptureRegistry {
    model_title: String,
    captures: BTreeMap<String, Arc<CommandCapture>>,
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

    pub(super) fn register(
        &mut self,
        id: &str,
        separate_output: bool,
    ) -> Result<Arc<CommandCapture>> {
        if self.captures.contains_key(id) {
            bail!("command capture already exists for {id}");
        }
        let capture = Arc::new(CommandCapture::new(
            self.model_title.clone(),
            separate_output,
        ));
        self.captures.insert(id.to_owned(), Arc::clone(&capture));
        Ok(capture)
    }

    pub(super) fn record(&self, text: &str) -> Result<()> {
        if !text.is_empty()
            && let Some(id) = self.active.as_deref()
        {
            self.require(id)?.record(text);
        }
        Ok(())
    }

    pub(super) fn handle(&mut self, event: ProtocolEvent, screen_title: &str) -> Result<()> {
        match event {
            ProtocolEvent::Start(id) => {
                tracing::debug!(command_id = %id, "terminal command start marker");
                self.start(&id)
            }
            ProtocolEvent::End(id) => {
                tracing::debug!(command_id = %id, "terminal command end marker");
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
            bail!("command capture {id} started while {active} is active");
        }
        self.require(id)?.start();
        self.active = Some(id.to_owned());
        Ok(())
    }

    fn finish(&mut self, id: &str) -> Result<()> {
        if let Some(active) = self.active.as_deref()
            && active != id
        {
            bail!("command capture {id} ended while {active} is active");
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

    fn require(&self, id: &str) -> Result<Arc<CommandCapture>> {
        self.captures
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("terminal marker references unknown command {id}"))
    }
}
