use alloc::sync::Arc;
use anyhow::Result;
use std::io::{Read, Write};
use std::sync::{Mutex, MutexGuard};
use std::thread;
#[cfg(unix)]
const CURSOR_POSITION_REPORT: &[u8] = b"\x1b[1;1R";
pub(super) fn start_reader(
    screen: Arc<Mutex<vt100::Parser>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    mut owned_reader: Box<dyn Read + Send>,
) {
    #[cfg(unix)]
    let response_writer = writer;
    #[cfg(not(unix))]
    drop(writer);
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        #[cfg(unix)]
        let mut responder = TerminalResponder::default();
        loop {
            match owned_reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read_len) => {
                    if let (Ok(mut parser), Some(chunk)) = (screen.lock(), buffer.get(..read_len)) {
                        parser.process(chunk);
                        #[cfg(unix)]
                        responder.answer_device_status_reports(chunk, &response_writer);
                    } else {
                        break;
                    }
                }
            }
        }
    });
}
#[cfg(unix)]
#[derive(Default)]
struct TerminalResponder {
    tail: Vec<u8>,
}
#[cfg(unix)]
impl TerminalResponder {
    fn answer_device_status_reports(
        &mut self,
        chunk: &[u8],
        writer: &Arc<Mutex<Box<dyn Write + Send>>>,
    ) {
        let mut bytes = self.tail.clone();
        bytes.extend_from_slice(chunk);
        if bytes.windows(4).any(|window| window == b"\x1b[6n")
            && let Ok(mut locked_writer) = writer.lock()
            && let Err(error) = locked_writer
                .write_all(CURSOR_POSITION_REPORT)
                .and_then(|()| locked_writer.flush())
        {
            eprintln!("failed to answer terminal cursor position request: {error}");
        }
        self.tail = bytes
            .get(bytes.len().saturating_sub(3)..)
            .unwrap_or_default()
            .to_vec();
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
