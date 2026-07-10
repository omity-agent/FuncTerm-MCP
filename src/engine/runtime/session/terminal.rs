use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use rmux_pty::PtyIo;
use std::sync::{Mutex, MutexGuard};
use std::thread::JoinHandle;
use tastty_core::{HostProfile, Parser, host_reply::auto_reply_bytes};
pub(super) type TerminalParser = Parser;
pub(super) type TerminalWriter = Arc<Mutex<PtyIo>>;
pub(super) fn start_reader(
    screen: Arc<Mutex<TerminalParser>>,
    writer: TerminalWriter,
    reader: PtyIo,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("functerm-pty-reader".to_owned())
        .spawn(move || {
            release_startup_guard(&reader);
            let host = HostProfile::default();
            let mut buffer = [0_u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(error) if is_pty_eof(&error) => break,
                    Err(error) => {
                        eprintln!("failed to read pty output: {error}");
                        break;
                    }
                    Ok(read_len) => {
                        let Some(chunk) = buffer.get(..read_len) else {
                            eprintln!("pty returned an invalid output length: {read_len}");
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
                                    .flatten()
                                    .collect::<Vec<u8>>()
                            }
                            Err(error) => {
                                eprintln!("screen mutex poisoned while reading pty: {error}");
                                break;
                            }
                        };
                        if let Err(error) = write_replies(&writer, &replies) {
                            eprintln!("failed to answer terminal host query: {error:#}");
                            break;
                        }
                    }
                }
            }
        })
}
pub(super) fn screen_title(parser: &TerminalParser) -> String {
    parser.screen().title().to_owned()
}
fn write_replies(writer: &TerminalWriter, replies: &[u8]) -> Result<()> {
    if replies.is_empty() {
        return Ok(());
    }
    lock_mutex(writer, "pty writer")?
        .write_all(replies)
        .context("failed to write terminal host query reply")
}
pub(super) fn lock_mutex<'guard, T>(
    mutex: &'guard Mutex<T>,
    name: &str,
) -> Result<MutexGuard<'guard, T>> {
    mutex
        .lock()
        .map_err(|error| anyhow::anyhow!("{name} mutex poisoned: {error}"))
}
#[cfg(unix)]
fn release_startup_guard(reader: &PtyIo) {
    reader.release_startup_slave_guard();
}
#[cfg(not(unix))]
const fn release_startup_guard(_reader: &PtyIo) {}
#[cfg(unix)]
fn is_pty_eof(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(libc::EIO)
}
#[cfg(not(unix))]
const fn is_pty_eof(_error: &std::io::Error) -> bool {
    false
}
