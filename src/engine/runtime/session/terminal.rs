mod io;
mod output_events;
mod sync;
mod title;
pub(super) use self::io::start_reader;
use self::output_events::{OutputEvent, OutputParser};
use self::title::CaptureRegistry;
pub(super) use self::title::CommandTitle;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use parking_lot::{Condvar, Mutex};
use tastty_core::{HostProfile, Parser, host_reply::auto_reply_bytes};
pub(super) struct Terminal {
    model_title: String,
    state: Mutex<TerminalState>,
    changed: Condvar,
}
struct TerminalState {
    parser: Parser,
    protocol: OutputParser,
    win32_input: bool,
    captures: CaptureRegistry,
    revision: u64,
    reader_closed: bool,
    reader_failure: Option<String>,
}
impl Terminal {
    pub(super) fn new(
        size: tastty_core::TerminalSize,
        scrollback_len: usize,
        model_title: &str,
    ) -> Result<Self> {
        let mut parser = Parser::new(size, scrollback_len);
        parser.process(crate::contract::window_title_sequence(model_title)?.as_bytes());
        drop(parser.screen_mut().drain_events());
        Ok(Self {
            model_title: model_title.to_owned(),
            state: Mutex::new(TerminalState {
                parser,
                protocol: OutputParser::new(),
                win32_input: false,
                captures: CaptureRegistry::new(model_title.to_owned()),
                revision: 0,
                reader_closed: false,
                reader_failure: None,
            }),
            changed: Condvar::new(),
        })
    }
    pub(super) fn capture_title(&self, command_id: &str) -> Result<Arc<CommandTitle>> {
        let mut state = self.state.lock();
        if let Some(message) = state.reader_failure.as_deref() {
            bail!("cannot start command after terminal reader failure: {message}");
        }
        if state.reader_closed {
            bail!("cannot start command after terminal reader closed");
        }
        state.captures.register(command_id)
    }
    pub(super) fn contents(&self) -> String {
        self.state.lock().parser.screen().contents()
    }
    pub(super) fn model_title(&self) -> String {
        self.model_title.clone()
    }
    #[cfg(test)]
    fn raw_title(&self) -> String {
        self.state.lock().parser.screen().title().to_owned()
    }
    pub(super) fn process(&self, chunk: &[u8], host: &HostProfile) -> Result<Vec<HostReply>> {
        let mut state = self.state.lock();
        let processed = (|| {
            let replies = state.process(chunk, host)?;
            state.revision = state
                .revision
                .checked_add(1)
                .context("terminal output revision overflow")?;
            Ok(replies)
        })();
        match processed {
            Ok(replies) => {
                drop(state);
                self.changed.notify_all();
                Ok(replies)
            }
            Err(error) => {
                let message = format!("failed to process terminal output: {error:#}");
                state.captures.fail_all(&message);
                state.reader_closed = true;
                state.reader_failure = Some(message);
                drop(state);
                self.changed.notify_all();
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
            self.process_screen(segment, host, &mut replies);
            match event {
                Some(OutputEvent::InputMode(enabled)) => self.win32_input = enabled,
                Some(OutputEvent::Capture(protocol_event)) => {
                    self.captures
                        .handle(protocol_event, self.parser.screen().title())?;
                }
                None => {}
            }
            remaining = tail;
        }
        Ok(replies)
    }
    fn process_screen(&mut self, bytes: &[u8], host: &HostProfile, replies: &mut Vec<HostReply>) {
        self.parser.process(bytes);
        replies.extend(
            self.parser
                .screen_mut()
                .drain_events()
                .into_iter()
                .filter_map(|event| auto_reply_bytes(&event, host))
                .map(|reply_bytes| HostReply {
                    bytes: reply_bytes,
                    win32_input: self.win32_input,
                }),
        );
    }
}
#[cfg(test)]
#[path = "../../../../tests/unit/terminal/ordered_controls.rs"]
mod ordered_controls;
#[cfg(test)]
#[path = "../../../../tests/unit/terminal/title_capture.rs"]
mod tests;
