use super::Terminal;
use alloc::sync::Arc;
use std::io::Read;
use std::thread::{self, JoinHandle};
pub(in crate::engine::runtime::session) fn start_reader(
    terminal: Arc<Terminal>,
    mut owned_reader: Box<dyn Read + Send>,
    on_reader_exit: impl FnOnce() + Send + 'static,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("functerm-pty-reader".to_owned())
        .spawn(move || {
            read_pty(&terminal, &mut owned_reader);
            on_reader_exit();
        })
}
fn read_pty(terminal: &Terminal, owned_reader: &mut dyn Read) {
    let mut buffer = [0_u8; 8192];
    loop {
        let read_len = match owned_reader.read(&mut buffer) {
            Ok(0) => {
                terminal.reader_closed();
                return;
            }
            Ok(length) => length,
            Err(error) => {
                tracing::error!(%error, "failed to read PTY output");
                terminal.reader_failed(&format!("failed to read PTY output: {error}"));
                return;
            }
        };
        let Some(chunk) = buffer.get(..read_len) else {
            terminal.reader_failed("PTY reader returned an invalid byte count");
            tracing::error!(read_len, "PTY reader returned an invalid byte count");
            return;
        };
        tracing::trace!(output = ?String::from_utf8_lossy(chunk), "terminal PTY output");
        if let Err(error) = terminal.process(chunk) {
            tracing::error!(error = format!("{error:#}"), "failed to process PTY output");
            return;
        }
    }
}
