use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use parking_lot::{Condvar, Mutex};
use std::{path::Path, time::Instant};
pub(in crate::engine::runtime::session) struct PathWatch {
    signal: Arc<Signal>,
    _watcher: RecommendedWatcher,
}
#[derive(Default)]
struct Signal {
    failure: Mutex<Option<String>>,
    changed: Condvar,
}
impl PathWatch {
    pub(in crate::engine::runtime::session) fn new(directory: &Path) -> Result<Self> {
        let signal = Arc::new(Signal::default());
        let observed = Arc::clone(&signal);
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let mut failure = observed.failure.lock();
                if let Err(error) = event {
                    *failure = Some(error.to_string());
                }
                drop(failure);
                observed.changed.notify_all();
            })
            .context("failed to create filesystem watcher")?;
        watcher
            .watch(directory, RecursiveMode::NonRecursive)
            .with_context(|| format!("failed to watch directory {}", directory.display()))?;
        Ok(Self {
            signal,
            _watcher: watcher,
        })
    }
    pub(in crate::engine::runtime::session) fn wait(
        &self,
        limit: Duration,
        ready: impl Fn() -> Result<bool>,
    ) -> Result<bool> {
        let started = Instant::now();
        let mut failure = self.signal.failure.lock();
        let result = loop {
            if ready()? {
                break Ok(true);
            }
            if let Some(message) = failure.as_deref() {
                bail!("filesystem watcher failed: {message}");
            }
            let Some(remaining) = limit.checked_sub(started.elapsed()) else {
                break Ok(false);
            };
            if self
                .signal
                .changed
                .wait_for(&mut failure, remaining)
                .timed_out()
            {
                break ready();
            }
        };
        drop(failure);
        result
    }
    pub(in crate::engine::runtime::session) fn wake(&self) {
        let guard = self.signal.failure.lock();
        self.signal.changed.notify_all();
        drop(guard);
    }
}
pub(crate) fn wait_for_path(path: &Path, limit: Duration) -> Result<bool> {
    if path.try_exists()? {
        return Ok(true);
    }
    if limit.is_zero() {
        return Ok(false);
    }
    let parent = path.parent().context("watched path has no parent")?;
    fs_err::create_dir_all(parent)?;
    let watch = PathWatch::new(parent)?;
    watch.wait(limit, || Ok(path.try_exists()?))
}
