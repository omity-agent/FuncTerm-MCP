use super::Terminal;
use alloc::sync::Arc;
use std::io::{Read, Write};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use tastty_core::HostProfile;
pub(in crate::engine::runtime::session) fn start_reader(
    terminal: Arc<Terminal>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    mut owned_reader: Box<dyn Read + Send>,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("functerm-pty-reader".to_owned())
        .spawn(move || read_pty(&terminal, &writer, &mut owned_reader))
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
                terminal.reader_closed("PTY reader closed before command title capture completed");
                return;
            }
            Ok(length) => length,
            Err(error) => {
                terminal.reader_closed(&format!("failed to read PTY output: {error}"));
                eprintln!("failed to read PTY output: {error}");
                return;
            }
        };
        let Some(chunk) = buffer.get(..read_len) else {
            terminal.reader_closed("PTY reader returned an invalid byte count");
            eprintln!("PTY reader returned an invalid byte count: {read_len}");
            return;
        };
        match terminal.process(chunk, &host) {
            Ok(replies) => write_replies(writer, &replies),
            Err(error) => {
                eprintln!("failed to process PTY output: {error:#}");
                return;
            }
        }
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
