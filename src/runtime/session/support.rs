use alloc::sync::Arc;
use anyhow::Result;
use std::io::Read;
use std::sync::{Mutex, MutexGuard};
use std::thread;
pub(super) fn start_reader(
    screen: Arc<Mutex<vt100::Parser>>,
    mut owned_reader: Box<dyn Read + Send>,
) {
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            match owned_reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read_len) => {
                    if let (Ok(mut parser), Some(chunk)) = (screen.lock(), buffer.get(..read_len)) {
                        parser.process(chunk);
                    } else {
                        break;
                    }
                }
            }
        }
    });
}
pub(super) fn lock_mutex<'guard, T>(
    mutex: &'guard Mutex<T>,
    name: &str,
) -> Result<MutexGuard<'guard, T>> {
    mutex
        .lock()
        .map_err(|error| anyhow::anyhow!("{name} mutex poisoned: {error}"))
}
