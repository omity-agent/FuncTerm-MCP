mod activity;
mod capture;
mod emulation;
mod io;
mod markers;
mod registry;
mod rendering;
mod transcript;
pub(super) use self::capture::CommandCapture;
use self::emulation::{PtyResponder, WindowTitleChanges};
pub(super) use self::io::start_reader;
use self::registry::{CaptureEnd, CaptureRegistry};
use self::rendering::CommandScreen;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use bytestring::ByteString;
use event_listener::Event;
use parking_lot::Mutex;
use portable_pty::PtySize;
use std::io::Write;
use wezterm_escape_parser::{
    Action, CSI,
    csi::{Edit, EraseInDisplay},
    esc::{Esc, EscCode},
    parser::Parser,
};
pub(super) struct Terminal {
    state: Mutex<TerminalState>,
    changed: Event,
    lifetime_changed: Event,
}
struct TerminalState {
    screen: Arc<Mutex<CommandScreen>>,
    responder: PtyResponder,
    window_titles: WindowTitleChanges,
    parser: Parser,
    actions: Vec<Action>,
    captures: CaptureRegistry,
    visible_revision: u64,
    visible_contents: ByteString,
    reader_closed: bool,
    reader_failure: Option<String>,
    process_exited: bool,
    process_failure: Option<String>,
    session_stopped: bool,
}
impl Terminal {
    pub(super) fn new(
        size: PtySize,
        model_title: &str,
        writer: Arc<Mutex<Box<dyn Write + Send>>>,
    ) -> Result<Arc<Self>> {
        let initial_title = crate::contract::window_title_sequence(model_title)?;
        let responder = PtyResponder::new(writer);
        let mut screen = CommandScreen::new(size, Box::new(responder.clone()));
        screen.model.advance_bytes(initial_title.as_bytes());
        let window_titles = WindowTitleChanges::default();
        screen
            .model
            .set_notification_handler(Box::new(window_titles.clone()));
        let visible_contents = screen.visible_contents().into();
        Ok(Arc::new(Self {
            state: Mutex::new(TerminalState {
                screen: Arc::new(Mutex::new(screen)),
                responder,
                window_titles,
                parser: Parser::new(),
                actions: Vec::new(),
                captures: CaptureRegistry::new(model_title.to_owned()),
                visible_revision: 0,
                visible_contents,
                reader_closed: false,
                reader_failure: None,
                process_exited: false,
                process_failure: None,
                session_stopped: false,
            }),
            changed: Event::new(),
            lifetime_changed: Event::new(),
        }))
    }
    pub(super) fn capture_command(
        &self,
        command_id: &str,
        separate_output: bool,
    ) -> Result<Arc<CommandCapture>> {
        let mut state = self.state.lock();
        if let Some(message) = state.reader_failure.as_deref() {
            bail!("cannot start command after terminal reader failure: {message}");
        }
        if state.reader_closed {
            bail!("cannot start command after terminal reader closed");
        }
        let screen = Arc::clone(&state.screen);
        state.captures.register(command_id, separate_output, screen)
    }
    pub(super) fn contents(&self) -> ByteString {
        self.state.lock().visible_contents.clone()
    }
    pub(super) fn title(&self) -> String {
        self.state.lock().captures.title().to_owned()
    }
    pub(super) fn defer_window_titles(&self, id: &str) {
        self.state.lock().captures.defer_window_titles(id);
    }
    pub(super) fn complete_at_prompt(&self, id: &str) -> Result<()> {
        self.state.lock().captures.finish(id, CaptureEnd::Prompt)
    }
    pub(super) fn process(&self, chunk: &[u8]) -> Result<()> {
        let mut state = self.state.lock();
        let processed = state.process(chunk);
        if let Err(error) = processed.as_ref() {
            let message = format!("failed to process terminal output: {error:#}");
            state.captures.fail(&message);
            state.reader_closed = true;
            state.reader_failure = Some(message);
        }
        drop(state);
        self.changed.notify(usize::MAX);
        processed
    }
}
impl TerminalState {
    fn process(&mut self, chunk: &[u8]) -> Result<()> {
        let mut actions = core::mem::take(&mut self.actions);
        self.parser
            .parse(chunk, |action| action.append_to(&mut actions));
        #[expect(
            clippy::iter_with_drain,
            reason = "retain the parser buffer between PTY reads"
        )]
        let applied = self.apply_parsed(actions.drain(..));
        self.actions = actions;
        if matches!(applied, Ok(false)) {
            return Ok(());
        }
        let refreshed = self.refresh_visible();
        applied.and(refreshed)
    }
    fn apply_parsed(&mut self, actions: impl Iterator<Item = Action>) -> Result<bool> {
        let mut pending = Vec::new();
        let mut applied = false;
        for action in actions {
            if let Some(event) = markers::parse(&action)? {
                applied |= self.apply(core::mem::take(&mut pending))?;
                self.captures.handle(event)?;
            } else {
                let checkpoint = matches!(
                    action,
                    Action::OperatingSystemCommand(_)
                        | Action::Esc(Esc::Code(EscCode::FullReset))
                        | Action::CSI(CSI::Edit(Edit::EraseInDisplay(
                            EraseInDisplay::EraseDisplay
                        )))
                );
                pending.push(action);
                if checkpoint {
                    applied |= self.apply(core::mem::take(&mut pending))?;
                }
            }
        }
        applied |= self.apply(pending)?;
        Ok(applied)
    }
    fn refresh_visible(&mut self) -> Result<()> {
        let contents = self.screen.lock().visible_contents();
        if self.visible_contents != contents {
            self.visible_revision = self
                .visible_revision
                .checked_add(1)
                .context("terminal visible content revision overflow")?;
            self.visible_contents = contents.into();
        }
        Ok(())
    }
    fn apply(&mut self, actions: Vec<Action>) -> Result<bool> {
        if actions.is_empty() {
            return Ok(false);
        }
        self.screen.lock().apply(actions);
        self.responder.check()?;
        if let Some(title) = self.window_titles.take() {
            self.captures.record_window_title(title);
        }
        Ok(true)
    }
}
