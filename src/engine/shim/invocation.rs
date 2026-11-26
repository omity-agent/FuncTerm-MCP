use anyhow::{Context as _, Result};
use core::time::Duration;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as _};
use std::path::Path;
use std::process::{Child, ExitStatus};
use std::sync::mpsc;

pub(super) fn run_shell_until_exit(
    ready_file: &Path,
    spawn: impl FnOnce() -> Result<Child>,
    on_ready: impl FnOnce() -> Result<()>,
) -> Result<ExitStatus> {
    let parent = ready_file
        .parent()
        .context("nested ready path has no parent")?;
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |event: notify::Result<notify::Event>| {
            let _sent = tx.send(event.map(|_| ()));
        },
        Config::default(),
    )
    .context("failed to create nested shell startup watcher")?;
    watcher
        .watch(parent, RecursiveMode::NonRecursive)
        .with_context(|| {
            format!(
                "failed to watch nested shell directory {}",
                parent.display()
            )
        })?;
    let mut child = spawn()?;
    let ready = startup(&mut child, ready_file, &rx);
    drop(watcher);
    match ready {
        Ok(Some(status)) => return Ok(status),
        Ok(None) => {
            if let Err(error) = on_ready() {
                stop(&mut child);
                return Err(error);
            }
        }
        Err(error) => {
            stop(&mut child);
            return Err(error);
        }
    }
    child.wait().context("failed to wait for nested shell")
}
fn startup(
    child: &mut Child,
    ready_file: &Path,
    rx: &mpsc::Receiver<notify::Result<()>>,
) -> Result<Option<ExitStatus>> {
    loop {
        if let Some(status) = child.try_wait().context("failed to inspect nested shell")? {
            return Ok(Some(status));
        }
        if ready_file.try_exists()? {
            return Ok(None);
        }
        match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(Ok(())) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Ok(Err(error)) => return Err(error).context("nested shell startup watcher failed"),
            Err(error @ mpsc::RecvTimeoutError::Disconnected) => return Err(error.into()),
        }
    }
}
fn stop(child: &mut Child) {
    let stopped = (|| {
        child.kill()?;
        child.wait()
    })();
    if let Err(error) = stopped {
        tracing::error!("could not stop nested shell after failed activation");
        tracing::debug!(%error, "nested shell shutdown failure");
    }
}
