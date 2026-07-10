use alloc::sync::Arc;
use anyhow::Result;
use std::io::{Read, Write};
use std::sync::{Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use tastty_core::{HostProfile, Parser, host_reply::auto_reply_bytes};
pub(super) type TerminalParser = Parser;
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
