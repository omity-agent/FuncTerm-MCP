use alloc::sync::Arc;
use anyhow::{Result, bail};
use std::io::{Read, Write};
use std::sync::{Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use tastty_core::{
    HostProfile, Parser, ScreenEvent, SemanticPrompt, TerminalSize, host_reply::auto_reply_bytes,
};
pub(super) struct TerminalState {
    parser: Parser,
    visible_title: String,
    title_phase: TitlePhase,
}
enum TitlePhase {
    Startup,
    Live,
}
pub(super) fn create_parser(
    size: TerminalSize,
    scrollback_len: usize,
    initial_title: &str,
) -> Result<TerminalState> {
    if initial_title.chars().any(char::is_control) {
        bail!("terminal_initial_title must not contain control characters");
    }
    let mut parser = Parser::new(size, scrollback_len);
    set_parser_title(&mut parser, initial_title);
    drop(parser.screen_mut().drain_events());
    Ok(TerminalState {
        parser,
        visible_title: initial_title.to_owned(),
        title_phase: TitlePhase::Startup,
    })
}
fn set_parser_title(parser: &mut Parser, title: &str) {
    let mut title_sequence = Vec::new();
    title_sequence.extend_from_slice(b"\x1b]2;");
    title_sequence.extend_from_slice(title.as_bytes());
    title_sequence.extend_from_slice(b"\x1b\\");
    parser.process(&title_sequence);
}
pub(super) fn start_reader(
    terminal: Arc<Mutex<TerminalState>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    mut owned_reader: Box<dyn Read + Send>,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("functerm-pty-reader".to_owned())
        .spawn(move || {
            let host = HostProfile::default();
            let mut buffer = [0_u8; 8192];
            loop {
                match owned_reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read_len) => {
                        let Some(chunk) = buffer.get(..read_len) else {
                            break;
                        };
                        let replies = match terminal.lock() {
                            Ok(mut state) => state
                                .process(chunk)
                                .iter()
                                .filter_map(|event| auto_reply_bytes(event, &host))
                                .collect::<Vec<_>>(),
                            Err(_) => break,
                        };
                        write_replies(&writer, &replies);
                    }
                }
            }
        })
}
impl TerminalState {
    fn process(&mut self, bytes: &[u8]) -> Vec<ScreenEvent> {
        self.parser.process(bytes);
        let events = self.parser.screen_mut().drain_events();
        let mut live_title_changed = false;
        for event in &events {
            if matches!(
                event,
                ScreenEvent::ShellIntegration {
                    mark: SemanticPrompt::PromptEnd,
                }
            ) && matches!(self.title_phase, TitlePhase::Startup)
            {
                self.title_phase = TitlePhase::Live;
            } else if matches!(self.title_phase, TitlePhase::Live)
                && matches!(event, ScreenEvent::TitleChanged)
            {
                live_title_changed = true;
            }
        }
        if live_title_changed {
            self.visible_title = self.parser.screen().title().to_owned();
        }
        events
    }
    pub(super) fn contents(&self) -> String {
        self.parser.screen().contents()
    }
    pub(super) fn title(&self) -> &str {
        &self.visible_title
    }
}
fn write_replies(writer: &Arc<Mutex<Box<dyn Write + Send>>>, replies: &[Vec<u8>]) {
    if replies.is_empty() {
        return;
    }
    match writer.lock() {
        Ok(mut locked_writer) => {
            for reply in replies {
                if let Err(error) = locked_writer.write_all(reply) {
                    eprintln!("failed to answer terminal host query: {error}");
                    return;
                }
            }
            if let Err(error) = locked_writer.flush() {
                eprintln!("failed to flush terminal host query replies: {error}");
            }
        }
        Err(error) => eprintln!("terminal writer mutex poisoned while replying: {error}"),
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
#[path = "terminal/tests.rs"]
mod tests;
