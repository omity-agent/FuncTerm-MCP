use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use event_listener::{Event, Listener as _};
use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use parking_lot::Mutex;
use std::{path::Path, time::Instant};
pub(in crate::engine::runtime::session) struct PathWatch {
    signal: Arc<Signal>,
    _watcher: RecommendedWatcher,
}
#[derive(Default)]
struct Signal {
    failure: Mutex<Option<String>>,
    changed: Event,
}
impl PathWatch {
    pub(in crate::engine::runtime::session) fn new(
        directory: &Path,
        mode: RecursiveMode,
    ) -> Result<Self> {
        let signal = Arc::new(Signal::default());
        let observed = Arc::clone(&signal);
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let mut failure = observed.failure.lock();
                if let Err(error) = event {
                    *failure = Some(error.to_string());
                }
                drop(failure);
                observed.changed.notify(usize::MAX);
            })
            .context("failed to create filesystem watcher")?;
        watcher
            .watch(directory, mode)
            .with_context(|| format!("failed to watch directory {}", directory.display()))?;
        Ok(Self {
            signal,
            _watcher: watcher,
        })
    }
    pub(in crate::engine::runtime::session) async fn wait(
        &self,
        limit: Duration,
        ready: impl Fn() -> Result<bool>,
    ) -> Result<bool> {
        wait_until(&self.signal.changed, limit, || self.ready(&ready)).await
    }
    fn ready(&self, ready: &impl Fn() -> Result<bool>) -> Result<bool> {
        if ready()? {
            return Ok(true);
        }
        if let Some(message) = self.signal.failure.lock().as_deref() {
            bail!("filesystem watcher failed: {message}");
        }
        Ok(false)
    }
    fn wait_blocking(&self, limit: Duration, ready: impl Fn() -> Result<bool>) -> Result<bool> {
        let started = Instant::now();
        loop {
            let listener = self.signal.changed.listen();
            if self.ready(&ready)? {
                return Ok(true);
            }
            let Some(remaining) = limit.checked_sub(started.elapsed()) else {
                return Ok(false);
            };
            if listener.wait_timeout(remaining).is_none() {
                return self.ready(&ready);
            }
        }
    }
    pub(in crate::engine::runtime::session) fn wake(&self) {
        self.signal.changed.notify(usize::MAX);
    }
}
pub(super) async fn wait_until(
    changed: &Event,
    limit: Duration,
    mut ready: impl FnMut() -> Result<bool>,
) -> Result<bool> {
    let waiting = async {
        loop {
            let listener = changed.listen();
            if ready()? {
                return Ok(true);
            }
            listener.await;
        }
    };
    match tokio::time::timeout(limit, waiting).await {
        Ok(result) => result,
        Err(_elapsed) => ready(),
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
    let watch = PathWatch::new(parent, RecursiveMode::NonRecursive)?;
    watch.wait_blocking(limit, || Ok(path.try_exists()?))
}
