use anyhow::{Context as _, Result};
use named_lock::{NamedLock, NamedLockGuard};
#[derive(Debug)]
pub(crate) struct DaemonAlreadyRunning {
    service_name: String,
}
impl DaemonAlreadyRunning {
    #[inline]
    pub(crate) fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }
    #[inline]
    pub(crate) fn service_name(&self) -> &str {
        &self.service_name
    }
}
impl core::fmt::Display for DaemonAlreadyRunning {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "daemon is already running for IPC service {}",
            self.service_name
        )
    }
}
impl core::error::Error for DaemonAlreadyRunning {}
pub(crate) struct DaemonLock {
    _lock: NamedLock,
    _guard: NamedLockGuard,
}
pub(crate) fn acquire_instance(service_name: &str) -> Result<DaemonLock> {
    let lock = create_lock(service_name, "instance")?;
    match lock.try_lock() {
        Ok(guard) => Ok(DaemonLock {
            _lock: lock,
            _guard: guard,
        }),
        Err(named_lock::Error::WouldBlock) => Err(DaemonAlreadyRunning::new(service_name).into()),
        Err(error) => Err(error).context("failed to acquire daemon instance lock"),
    }
}
pub(crate) fn already_running_service_name(error: &anyhow::Error) -> Option<&str> {
    error.chain().find_map(|source| {
        source
            .downcast_ref::<DaemonAlreadyRunning>()
            .map(DaemonAlreadyRunning::service_name)
    })
}
pub(crate) fn acquire_startup(service_name: &str) -> Result<DaemonLock> {
    let lock = create_lock(service_name, "startup")?;
    let guard = lock
        .lock()
        .context("failed to acquire daemon startup lock")?;
    Ok(DaemonLock {
        _lock: lock,
        _guard: guard,
    })
}
fn create_lock(service_name: &str, kind: &str) -> Result<NamedLock> {
    let name = crate::runtime::transport::lock_name(service_name, kind);
    NamedLock::create(&name).with_context(|| format!("failed to create daemon {kind} lock"))
}
