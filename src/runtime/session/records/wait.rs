use super::CommandRecord;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as _};
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::time::Instant;
pub(in crate::runtime::session) fn wait_for_done(done: &Path, limit: Duration) -> Result<bool> {
    wait_for_path(done, limit)
}
pub(in crate::runtime::session) fn wait_for_start_or_done(
    record: &CommandRecord,
    limit: Duration,
) -> Result<bool> {
    wait_for_any_path(&[record.started.as_path(), record.done.as_path()], limit)
}
pub(in crate::runtime::session) fn wait_for_path(path: &Path, limit: Duration) -> Result<bool> {
    wait_for_any_path(&[path], limit)
}
fn wait_for_any_path(paths: &[&Path], limit: Duration) -> Result<bool> {
    if paths.iter().any(|path| path.exists()) {
        return Ok(true);
    }
    if limit.is_zero() {
        return Ok(false);
    }
    let first_path = paths.first().context("no watched paths")?;
    let parent = first_path.parent().context("watched path has no parent")?;
    for path in paths {
        if path.parent() != Some(parent) {
            bail!("watched paths must share a parent directory");
        }
    }
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create watched parent directory {}",
            parent.display()
        )
    })?;
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default())
        .context("failed to create filesystem watcher")?;
    watcher
        .watch(parent, RecursiveMode::NonRecursive)
        .with_context(|| format!("failed to watch directory {}", parent.display()))?;
    if paths.iter().any(|path| path.exists()) {
        return Ok(true);
    }
    let start = Instant::now();
    loop {
        let Some(remaining) = limit.checked_sub(start.elapsed()) else {
            return Ok(false);
        };
        match rx.recv_timeout(remaining) {
            Ok(Ok(_event)) => {
                if paths.iter().any(|path| path.exists()) {
                    return Ok(true);
                }
            }
            Ok(Err(error)) => return Err(error).context("filesystem watcher failed"),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Ok(paths.iter().any(|path| path.exists()));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                bail!("filesystem watcher disconnected");
            }
        }
    }
}
