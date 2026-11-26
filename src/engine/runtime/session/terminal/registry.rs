use super::{capture::CommandCapture, markers::ProtocolEvent, rendering::CommandScreen};
use alloc::sync::Arc;
use anyhow::{Context as _, Result, ensure};
use parking_lot::Mutex;
pub(super) struct CaptureRegistry {
    title: String,
    current: Option<RegisteredCapture>,
}
struct RegisteredCapture {
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
            current: None,
        }
    }
    pub(super) fn register(
        &mut self,
        id: &str,
        separate_output: bool,
        screen: Arc<Mutex<CommandScreen>>,
    ) -> Result<Arc<CommandCapture>> {
        ensure!(
            self.current.is_none(),
            "terminal already has a registered command capture"
        );
        let capture = Arc::new(CommandCapture::new(
            self.title.clone(),
            separate_output,
            screen,
        ));
        self.current = Some(RegisteredCapture {
            id: id.to_owned(),
            capture: Arc::clone(&capture),
            titles: TitleUpdates::Live,
        });
        tracing::trace!(command_id = %id, "terminal command capture registered");
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
                if let Some(current) = self.current.as_ref() {
                    current.capture.observe_prompt(cwd);
                }
                Ok(())
            }
        }
    }
    pub(super) fn fail(&mut self, message: &str) {
        if let Some(current) = self.current.take() {
            current.capture.fail(message);
        }
    }
    pub(super) fn freeze(&mut self) -> Result<()> {
        if let Some(current) = self.current.as_ref() {
            current.capture.freeze()?;
        }
        self.current = None;
        Ok(())
    }
    fn start(&mut self, id: &str) -> Result<()> {
        self.require(id)?.capture.start()
    }
    pub(super) fn finish(&mut self, id: &str, end: CaptureEnd) -> Result<()> {
        let current = self.require(id)?;
        if matches!(end, CaptureEnd::Marker)
            && let TitleUpdates::Deferred(title) = core::mem::take(&mut current.titles)
        {
            current.capture.update_title(&title);
        }
        self.title = current.capture.cancel()?;
        self.current = None;
        Ok(())
    }
    pub(super) fn title(&self) -> &str {
        &self.title
    }
    pub(super) fn defer_window_titles(&mut self, id: &str) {
        if let Some(current) = self.current.as_mut()
            && current.id == id
            && current.capture.active()
            && matches!(current.titles, TitleUpdates::Live)
        {
            current.titles = TitleUpdates::AwaitingEnd;
        }
    }
    pub(super) fn record_window_title(&mut self, title: String) {
        let Some(current) = self.current.as_mut() else {
            return;
        };
        if matches!(current.titles, TitleUpdates::Live) {
            if current.capture.update_title(&title) {
                self.title = title;
            }
        } else {
            current.titles = TitleUpdates::Deferred(title);
        }
    }
    fn require(&mut self, id: &str) -> Result<&mut RegisteredCapture> {
        self.current
            .as_mut()
            .filter(|current| current.id == id)
            .with_context(|| format!("terminal marker references unknown command {id}"))
    }
}
