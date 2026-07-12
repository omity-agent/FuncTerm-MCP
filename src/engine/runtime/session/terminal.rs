use alloc::sync::Arc;
use anyhow::{Result, bail};
use std::io::{Read, Write};
use std::sync::{Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use tastty_core::{HostProfile, Parser, host_reply::auto_reply_bytes};
pub(super) type TerminalParser = Parser;
pub(super) fn create_parser(
    size: tastty_core::TerminalSize,
    scrollback_len: usize,
    initial_title: &str,
) -> Result<TerminalParser> {
    if initial_title.chars().any(char::is_control) {
        bail!("terminal_initial_title must not contain control characters");
    }
    let mut parser = TerminalParser::new(size, scrollback_len);
    let mut title_sequence = Vec::new();
    title_sequence.extend_from_slice(b"\x1b]2;");
    title_sequence.extend_from_slice(initial_title.as_bytes());
    title_sequence.extend_from_slice(b"\x1b\\");
    parser.process(&title_sequence);
    drop(parser.screen_mut().drain_events());
    Ok(parser)
}
pub(super) fn start_reader(
    screen: Arc<Mutex<TerminalParser>>,
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
                        let replies = match screen.lock() {
                            Ok(mut parser) => {
                                parser.process(chunk);
                                parser
                                    .screen_mut()
                                    .drain_events()
                                    .into_iter()
                                    .filter_map(|event| auto_reply_bytes(&event, &host))
                                    .collect::<Vec<_>>()
                            }
                            Err(_) => break,
                        };
                        write_replies(&writer, &replies);
                    }
                }
            }
        })
}
pub(super) fn screen_title(parser: &TerminalParser) -> String {
    parser.screen().title().to_owned()
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
mod tests {
    use super::create_parser;
    use tastty_core::TerminalSize;
    const SIZE: TerminalSize = TerminalSize {
        rows: 30,
        cols: 120,
    };
    #[test]
    fn shell_title_overwrites_initial_title() {
        let mut parser = create_parser(SIZE, 0, "FuncTerm").unwrap();
        assert_eq!(parser.screen().title(), "FuncTerm");
        parser.process(b"\x1b]2;Shell title\x1b\\");
        assert_eq!(parser.screen().title(), "Shell title");
    }
    #[test]
    fn control_characters_are_rejected() {
        let result = create_parser(SIZE, 0, "unsafe\x1btitle");
        let Err(error) = result else {
            panic!("control characters should be rejected");
        };
        assert_eq!(
            error.to_string(),
            "terminal_initial_title must not contain control characters"
        );
    }
}
