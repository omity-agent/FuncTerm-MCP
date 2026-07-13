mod io;
mod sequence;
mod title;
pub(super) use self::io::start_reader;
use self::sequence::ProtocolParser;
use self::title::CaptureRegistry;
pub(super) use self::title::CommandTitle;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use std::sync::{Mutex, MutexGuard};
use tastty_core::{HostProfile, Parser, host_reply::auto_reply_bytes};
pub(super) struct Terminal {
    state: Mutex<TerminalState>,
}
struct TerminalState {
    parser: Parser,
    protocol: ProtocolParser,
    captures: CaptureRegistry,
}
impl Terminal {
    pub(super) fn new(
        size: tastty_core::TerminalSize,
        scrollback_len: usize,
        initial_title: &str,
    ) -> Result<Self> {
        if initial_title.chars().any(char::is_control) {
            bail!("terminal_initial_title must not contain control characters");
        }
        let mut parser = Parser::new(size, scrollback_len);
        parser.process(title_sequence(initial_title).as_bytes());
        drop(parser.screen_mut().drain_events());
        Ok(Self {
            state: Mutex::new(TerminalState {
                parser,
                protocol: ProtocolParser::new(),
                captures: CaptureRegistry::new(initial_title.to_owned()),
            }),
        })
    }
    pub(super) fn capture_title(&self, command_id: &str) -> Result<Arc<CommandTitle>> {
        self.lock()?.captures.register(command_id)
    }
    pub(super) fn contents(&self) -> Result<String> {
        Ok(self.lock()?.parser.screen().contents())
    }
    pub(super) fn title(&self) -> Result<String> {
        Ok(self.lock()?.parser.screen().title().to_owned())
    }
    pub(super) fn process(&self, chunk: &[u8], host: &HostProfile) -> Result<Vec<Vec<u8>>> {
        let mut state = self.lock()?;
        match state.process(chunk, host) {
            Ok(replies) => {
                drop(state);
                Ok(replies)
            }
            Err(error) => {
                state
                    .captures
                    .fail_all(&format!("failed to process terminal output: {error:#}"));
                drop(state);
                Err(error)
            }
        }
    }
    pub(super) fn reader_closed(&self, message: &str) {
        match self.lock() {
            Ok(mut state) => state.captures.fail_all(message),
            Err(error) => eprintln!("failed to close command title captures: {error:#}"),
        }
    }
    fn lock(&self) -> Result<MutexGuard<'_, TerminalState>> {
        self.state
            .lock()
            .map_err(|error| anyhow::anyhow!("terminal mutex poisoned: {error}"))
    }
}
impl TerminalState {
    fn process(&mut self, chunk: &[u8], host: &HostProfile) -> Result<Vec<Vec<u8>>> {
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
            self.process_screen(segment, host, &mut replies);
            let screen_title = self.parser.screen().title().to_owned();
            self.captures.handle(protocol_event, &screen_title)?;
            parsed_end = protocol_end;
        }
        let tail = chunk
            .get(parsed_end..)
            .context("terminal tail offset exceeds PTY output")?;
        self.process_screen(tail, host, &mut replies);
        Ok(replies)
    }
    fn process_screen(&mut self, bytes: &[u8], host: &HostProfile, replies: &mut Vec<Vec<u8>>) {
        self.parser.process(bytes);
        replies.extend(
            self.parser
                .screen_mut()
                .drain_events()
                .into_iter()
                .filter_map(|event| auto_reply_bytes(&event, host)),
        );
    }
}
fn title_sequence(title: &str) -> String {
    format!("\x1b]2;{title}\x1b\\")
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
