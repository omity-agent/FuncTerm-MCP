use anyhow::{Context as _, Result};
use fs2::FileExt as _;
use std::fs::{File, OpenOptions};
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
    file: File,
}
impl Drop for DaemonLock {
    fn drop(&mut self) {
        if let Err(error) = self.file.unlock() {
            eprintln!("failed to unlock daemon lock: {error}");
        }
    }
}
pub(crate) fn acquire_instance(service_name: &str) -> Result<DaemonLock> {
    let file = open_lock_file(service_name, "instance.lock")?;
    match file.try_lock_exclusive() {
        Ok(()) => Ok(DaemonLock { file }),
        Err(error) if is_lock_contended(&error) => {
            Err(DaemonAlreadyRunning::new(service_name).into())
        }
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
    let file = open_lock_file(service_name, "startup.lock")?;
    file.lock_exclusive()
        .context("failed to acquire daemon startup lock")?;
    Ok(DaemonLock { file })
}
fn open_lock_file(service_name: &str, name: &str) -> Result<File> {
    let path = crate::runtime::temp::daemon_root()?
        .join("locks")
        .join(hex::encode(service_name))
        .join(name);
    let parent = path
        .parent()
        .with_context(|| format!("lock path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create daemon lock directory {}",
            parent.display()
        )
    })?;
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("failed to open daemon lock {}", path.display()))
}
#[expect(
    clippy::std_instead_of_core,
    reason = "Error and ErrorKind are only available from std"
)]
fn is_lock_contended(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock || error.raw_os_error() == Some(33)
}
