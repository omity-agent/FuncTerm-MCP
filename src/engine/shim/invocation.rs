use anyhow::{Context as _, Result};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as _};
use std::path::Path;
use std::process::ExitStatus;
use std::sync::mpsc;
use std::thread;
pub(super) fn run_shell_until_exit(
    mut child: std::process::Child,
    ready_file: &Path,
    on_ready: impl FnOnce() -> Result<()>,
) -> Result<ExitStatus> {
    let parent = ready_file
        .parent()
        .context("nested ready path has no parent")?;
    let (tx, rx) = mpsc::channel();
    let notify_tx = tx.clone();
    let mut watcher = RecommendedWatcher::new(
        move |event: notify::Result<notify::Event>| {
            let _sent = notify_tx.send(StartupEvent::Filesystem(event.map(|_| ())));
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
    let wait_tx = tx;
    thread::spawn(move || {
        let status = child.wait().context("failed to wait for nested shell");
        let _sent = wait_tx.send(StartupEvent::Exited(status));
    });
    let mut ready_handler = Some(on_ready);
    if ready_file.exists()
        && let Some(handler) = ready_handler.take()
    {
        handler()?;
    }
    loop {
        match rx
            .recv()
            .context("nested shell startup watcher disconnected")?
        {
            StartupEvent::Filesystem(Ok(())) => {
                if ready_file.exists()
                    && let Some(handler) = ready_handler.take()
                {
                    handler()?;
                }
            }
            StartupEvent::Filesystem(Err(error)) => return Err(error).context("watcher failed"),
            StartupEvent::Exited(status) => return status,
        }
    }
}
enum StartupEvent {
    Filesystem(notify::Result<()>),
    Exited(Result<ExitStatus>),
}
