mod activity;
mod capture;
mod emulation;
mod io;
mod markers;
mod registry;
mod rendering;
mod transcript;
pub(super) use self::capture::CommandCapture;
use self::emulation::PtyResponder;
pub(super) use self::io::start_reader;
use self::registry::CaptureRegistry;
use self::rendering::CommandScreen;
use alloc::sync::{Arc, Weak};
use anyhow::{Context as _, Result, bail};
use bytestring::ByteString;
use event_listener::Event;
use parking_lot::Mutex;
use portable_pty::PtySize;
use std::io::Write;
use wezterm_escape_parser::{Action, parser::Parser};
pub(super) struct Terminal {
    model_title: String,
    state: Mutex<TerminalState>,
    changed: Event,
}
struct TerminalState {
    screen: Arc<Mutex<CommandScreen>>,
    screen_title: String,
    parser: Parser,
    captures: CaptureRegistry,
    visible_revision: u64,
    visible_contents: ByteString,
    reader_closed: bool,
    reader_failure: Option<String>,
    process_exited: bool,
}
impl Terminal {
    pub(super) fn new(
        size: PtySize,
        model_title: &str,
        writer: Arc<Mutex<Box<dyn Write + Send>>>,
    ) -> Result<Arc<Self>> {
        let initial_title = crate::contract::window_title_sequence(model_title)?;
        Ok(Arc::new_cyclic(|terminal| {
            let responder = PtyResponder::new(writer, Weak::clone(terminal));
            let mut screen = CommandScreen::new(size, Box::new(responder));
            screen.model.advance_bytes(initial_title.as_bytes());
            let visible_contents = screen.visible_contents().into();
            Self {
                model_title: model_title.to_owned(),
                state: Mutex::new(TerminalState {
                    screen: Arc::new(Mutex::new(screen)),
                    screen_title: model_title.to_owned(),
                    parser: Parser::new(),
                    captures: CaptureRegistry::new(model_title.to_owned()),
                    visible_revision: 0,
                    visible_contents,
                    reader_closed: false,
                    reader_failure: None,
                    process_exited: false,
                }),
                changed: Event::new(),
            }
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
    pub(super) fn model_title(&self) -> String {
        self.model_title.clone()
    }
    pub(super) fn complete_at_prompt(&self, id: &str) -> Result<()> {
        self.state.lock().captures.finish(id)
    }
    pub(super) fn process(&self, chunk: &[u8]) -> Result<()> {
        let mut state = self.state.lock();
        let processed = state.process(chunk);
        if let Err(error) = processed.as_ref() {
            let message = format!("failed to process terminal output: {error:#}");
            state.captures.fail_all(&message);
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
        let mut actions = Vec::new();
        self.parser
            .parse(chunk, |action| action.append_to(&mut actions));
        let mut pending = Vec::new();
        for action in actions {
            if let Some(event) = markers::parse(&action)? {
                self.apply(core::mem::take(&mut pending))?;
                self.captures.handle(event)?;
            } else {
                let checkpoint = matches!(
                    action,
                    Action::CSI(_) | Action::Esc(_) | Action::OperatingSystemCommand(_)
                );
                pending.push(action);
                if checkpoint {
                    self.apply(core::mem::take(&mut pending))?;
                }
            }
        }
        self.apply(pending)?;
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
    fn apply(&mut self, actions: Vec<Action>) -> Result<()> {
        if actions.is_empty() {
            return Ok(());
        }
        let mut screen = self.screen.lock();
        screen.apply(actions);
        if self.screen_title != screen.model.get_title() {
            screen.model.get_title().clone_into(&mut self.screen_title);
            drop(screen);
            self.captures.update(&self.screen_title)?;
        }
        Ok(())
    }
}
