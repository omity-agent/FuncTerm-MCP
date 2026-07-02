use anyhow::{Context as _, Result, bail};
use fs2::FileExt as _;
use std::fs::{File, OpenOptions};
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
            bail!("daemon is already running for IPC service {service_name}")
        }
        Err(error) => Err(error).context("failed to acquire daemon instance lock"),
    }
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
    reason = "core::io is unstable; Error and ErrorKind are only available from std"
)]
fn is_lock_contended(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock
}
