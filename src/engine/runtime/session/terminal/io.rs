use super::Terminal;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use parking_lot::Mutex;
use std::io::{Read, Write};
use std::thread::{self, JoinHandle};
use tastty_core::HostProfile;
pub(in crate::engine::runtime::session) fn start_reader(
    terminal: Arc<Terminal>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    mut owned_reader: Box<dyn Read + Send>,
    on_reader_exit: impl FnOnce() + Send + 'static,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("functerm-pty-reader".to_owned())
        .spawn(move || {
            read_pty(&terminal, &writer, &mut owned_reader);
            on_reader_exit();
        })
}
fn read_pty(
    terminal: &Terminal,
    writer: &Arc<Mutex<Box<dyn Write + Send>>>,
    owned_reader: &mut dyn Read,
) {
    let host = HostProfile::default();
    let mut buffer = [0_u8; 8192];
    loop {
        let read_len = match owned_reader.read(&mut buffer) {
            Ok(0) => {
                terminal.reader_closed();
                return;
            }
            Ok(length) => length,
            Err(error) => {
                terminal.reader_failed(&format!("failed to read PTY output: {error}"));
                eprintln!("failed to read PTY output: {error}");
                return;
            }
        };
        let Some(chunk) = buffer.get(..read_len) else {
            terminal.reader_failed("PTY reader returned an invalid byte count");
            eprintln!("PTY reader returned an invalid byte count: {read_len}");
            return;
        };
        tracing::trace!(output = ?String::from_utf8_lossy(chunk), "terminal PTY output");
        match terminal.process(chunk, &host) {
            Ok(replies) => {
                if let Err(error) = write_replies(writer, &replies) {
                    let message = format!("failed to answer terminal host query: {error:#}");
                    terminal.reader_failed(&message);
                    eprintln!("{message}");
                    return;
                }
            }
            Err(error) => {
                eprintln!("failed to process PTY output: {error:#}");
                return;
            }
        }
    }
}
fn write_replies(writer: &Arc<Mutex<Box<dyn Write + Send>>>, replies: &[Vec<u8>]) -> Result<()> {
    if replies.is_empty() {
        return Ok(());
    }
    let mut locked_writer = writer.lock();
    for reply in replies {
        tracing::debug!(bytes = ?reply, "writing terminal host query reply");
        locked_writer
            .write_all(reply)
            .context("failed to write terminal host query reply")?;
    }
    locked_writer
        .flush()
        .context("failed to flush terminal host query replies")
}
