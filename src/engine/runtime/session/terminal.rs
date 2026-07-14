mod input_mode;
mod io;
mod sequence;
mod sync;
mod title;
use self::input_mode::InputModeTracker;
pub(super) use self::io::start_reader;
use self::sequence::ProtocolParser;
use self::title::CaptureRegistry;
pub(super) use self::title::CommandTitle;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use std::sync::{Condvar, Mutex, MutexGuard};
use tastty_core::{HostProfile, Parser, host_reply::auto_reply_bytes};
pub(super) struct Terminal {
    model_title: String,
    state: Mutex<TerminalState>,
    changed: Condvar,
}
struct TerminalState {
    parser: Parser,
    protocol: ProtocolParser,
    input_mode: InputModeTracker,
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
                protocol: ProtocolParser::new(),
                input_mode: InputModeTracker::new(),
                captures: CaptureRegistry::new(model_title.to_owned()),
                revision: 0,
                reader_closed: false,
                reader_failure: None,
            }),
            changed: Condvar::new(),
        })
    }
    pub(super) fn capture_title(&self, command_id: &str) -> Result<Arc<CommandTitle>> {
        self.lock()?.captures.register(command_id)
    }
    pub(super) fn contents(&self) -> Result<String> {
        Ok(self.lock()?.parser.screen().contents())
    }
    pub(super) fn model_title(&self) -> String {
        self.model_title.clone()
    }
    #[cfg(test)]
    fn raw_title(&self) -> Result<String> {
        Ok(self.lock()?.parser.screen().title().to_owned())
    }
    pub(super) fn process(&self, chunk: &[u8], host: &HostProfile) -> Result<Vec<HostReply>> {
        let mut state = self.lock()?;
        let processed = state.process(chunk, host).and_then(|replies| {
            state.revision = state
                .revision
                .checked_add(1)
                .context("terminal output revision overflow")?;
            Ok(replies)
        });
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
    fn lock(&self) -> Result<MutexGuard<'_, TerminalState>> {
        self.state
            .lock()
            .map_err(|error| anyhow::anyhow!("terminal mutex poisoned: {error}"))
    }
}
pub(super) struct HostReply {
    bytes: Vec<u8>,
    win32_input: bool,
}
impl TerminalState {
    fn process(&mut self, chunk: &[u8], host: &HostProfile) -> Result<Vec<HostReply>> {
        let mut replies = Vec::new();
        let mut parsed_end = 0_usize;
        let mut protocol_end = 0_usize;
        while protocol_end < chunk.len() {
            let remaining = chunk
                .get(protocol_end..)
                .context("terminal protocol offset exceeds PTY output")?;
            let (consumed, event) = self.protocol.advance(remaining);
            if consumed == 0 {
                bail!("terminal protocol parser made no progress");
            }
            protocol_end = protocol_end
                .checked_add(consumed)
                .context("terminal protocol offset overflow")?;
            let Some(protocol_event) = event else {
                continue;
            };
            let segment = chunk
                .get(parsed_end..protocol_end)
                .context("terminal event offset exceeds PTY output")?;
            self.process_screen(segment, host, &mut replies)?;
            let screen_title = self.parser.screen().title().to_owned();
            self.captures.handle(protocol_event, &screen_title)?;
            parsed_end = protocol_end;
        }
        let tail = chunk
            .get(parsed_end..)
            .context("terminal tail offset exceeds PTY output")?;
        self.process_screen(tail, host, &mut replies)?;
        Ok(replies)
    }
    fn process_screen(
        &mut self,
        bytes: &[u8],
        host: &HostProfile,
        replies: &mut Vec<HostReply>,
    ) -> Result<()> {
        let parser = &mut self.parser;
        self.input_mode
            .process_segments(bytes, |segment, win32_input| {
                parser.process(segment);
                replies.extend(
                    parser
                        .screen_mut()
                        .drain_events()
                        .into_iter()
                        .filter_map(|event| auto_reply_bytes(&event, host))
                        .map(|reply_bytes| HostReply {
                            bytes: reply_bytes,
                            win32_input,
                        }),
                );
            })
    }
}
pub(super) fn lock_mutex<'guard, T>(
    mutex: &'guard Mutex<T>,
    name: &str,
) -> Result<MutexGuard<'guard, T>> {
    mutex
        .lock()
        .map_err(|error| anyhow::anyhow!("{name} mutex poisoned: {error}"))
}
#[cfg(test)]
#[path = "terminal/terminal_tests.rs"]
mod tests;
