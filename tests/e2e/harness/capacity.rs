use super::command::exe;
use core::time::Duration;
use named_lock::{NamedLock, NamedLockGuard};
const MAX_PARALLEL_DAEMONS: usize = 2;
pub(super) struct DaemonPermit {
    _guard: NamedLockGuard,
}
impl DaemonPermit {
    pub(super) fn acquire() -> Self {
        let suite = blake3::hash(exe().as_os_str().as_encoded_bytes());
        let locks: Vec<_> = (0..MAX_PARALLEL_DAEMONS)
            .map(|slot| NamedLock::create(&format!("functerm-test-{suite}-{slot}")).unwrap())
            .collect();
        loop {
            for lock in &locks {
                match lock.try_lock() {
                    Ok(guard) => return Self { _guard: guard },
                    Err(named_lock::Error::WouldBlock) => {}
                    Err(error) => panic!("failed to acquire test daemon slot: {error}"),
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
