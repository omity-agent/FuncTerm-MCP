mod activity;
mod capture;
mod io;
mod output_events;
mod registry;
mod rendering;
pub(super) use self::capture::CommandCapture;
pub(super) use self::io::start_reader;
use self::output_events::{OutputEvent, OutputParser};
use self::registry::CaptureRegistry;
use self::rendering::CommandScreen;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use event_listener::Event;
use parking_lot::Mutex;
use tastty_core::{HostProfile, host_reply::auto_reply_bytes};
pub(super) struct Terminal {
    model_title: String,
    state: Mutex<TerminalState>,
    changed: Event,
}
struct TerminalState {
    screen: Arc<Mutex<CommandScreen>>,
    protocol: OutputParser,
    win32_input: bool,
    captures: CaptureRegistry,
    visible_revision: u64,
    visible_contents: String,
    reader_closed: bool,
    reader_failure: Option<String>,
    process_exited: bool,
}
impl Terminal {
    pub(super) fn new(size: tastty_core::TerminalSize, model_title: &str) -> Result<Self> {
        let mut screen = CommandScreen::new(size);
        screen
            .parser
            .process(crate::contract::window_title_sequence(model_title)?.as_bytes());
        drop(screen.parser.screen_mut().drain_events());
        let visible_contents = screen.parser.screen().contents();
        screen.parser.screen_mut().clear_dirty();
        Ok(Self {
            model_title: model_title.to_owned(),
            state: Mutex::new(TerminalState {
                screen: Arc::new(Mutex::new(screen)),
                protocol: OutputParser::new(),
                win32_input: false,
                captures: CaptureRegistry::new(model_title.to_owned()),
                visible_revision: 0,
                visible_contents,
                reader_closed: false,
                reader_failure: None,
                process_exited: false,
            }),
            changed: Event::new(),
        })
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
    pub(super) fn contents(&self) -> String {
        self.state.lock().visible_contents.clone()
    }
    pub(super) fn model_title(&self) -> String {
        self.model_title.clone()
    }
    pub(super) fn process(&self, chunk: &[u8], host: &HostProfile) -> Result<Vec<HostReply>> {
        let mut state = self.state.lock();
        let processed = (|| {
            let replies = state.process(chunk, host)?;
            let shared_screen = Arc::clone(&state.screen);
            let mut screen = shared_screen.lock();
            if screen.parser.screen().dirty_rows().next().is_some() {
                let contents = screen.parser.screen().contents();
                if contents != state.visible_contents {
                    state.visible_revision = state
                        .visible_revision
                        .checked_add(1)
                        .context("terminal visible content revision overflow")?;
                    state.visible_contents = contents;
                }
                screen.parser.screen_mut().clear_dirty();
            }
            drop(screen);
            Ok(replies)
        })();
        match processed {
            Ok(replies) => {
                drop(state);
                self.changed.notify(usize::MAX);
                Ok(replies)
            }
            Err(error) => {
                let message = format!("failed to process terminal output: {error:#}");
                state.captures.fail_all(&message);
                state.reader_closed = true;
                state.reader_failure = Some(message);
                drop(state);
                self.changed.notify(usize::MAX);
                Err(error)
            }
        }
    }
}
pub(super) struct HostReply {
    bytes: Vec<u8>,
    win32_input: bool,
}
impl TerminalState {
    fn process(&mut self, chunk: &[u8], host: &HostProfile) -> Result<Vec<HostReply>> {
        let mut replies = Vec::new();
        let mut remaining = chunk;
        while !remaining.is_empty() {
            let (consumed, event) = self.protocol.advance(remaining);
            if consumed == 0 {
                bail!("terminal protocol parser made no progress");
            }
            let (segment, tail) = remaining
                .split_at_checked(consumed)
                .context("terminal event offset exceeds PTY output")?;
            self.process_screen(segment, host, &mut replies)?;
            match event {
                Some(OutputEvent::InputMode(enabled)) => self.win32_input = enabled,
                Some(OutputEvent::Capture(protocol_event)) => {
                    let title = self.screen.lock().parser.screen().title().to_owned();
                    self.captures.handle(protocol_event, &title)?;
                }
                Some(OutputEvent::ScreenCheckpoint) | None => {}
            }
            remaining = tail;
        }
        Ok(replies)
    }
    fn process_screen(
        &self,
        bytes: &[u8],
        host: &HostProfile,
        replies: &mut Vec<HostReply>,
    ) -> Result<()> {
        let mut screen = self.screen.lock();
        screen.parser.process(bytes);
        for event in screen.parser.screen_mut().drain_events() {
            screen.observe(&event)?;
            if let Some(reply_bytes) = auto_reply_bytes(&event, host) {
                replies.push(HostReply {
                    bytes: reply_bytes,
                    win32_input: self.win32_input,
                });
            }
        }
        drop(screen);
        Ok(())
    }
}
