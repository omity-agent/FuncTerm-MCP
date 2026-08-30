use crate::runtime::session::terminal::Terminal;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as _};
use portable_pty::Child;
use std::path::Path;
use std::sync::mpsc;
use std::time::Instant;
#[cfg(windows)]
mod windows;
pub(super) struct StartupEvents {
    sender: mpsc::Sender<StartupEvent>,
    receiver: mpsc::Receiver<StartupEvent>,
}
impl StartupEvents {
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self { sender, receiver }
    }
    pub(super) fn reader_exit_notifier(&self) -> impl FnOnce() + Send + 'static {
        let sender = self.sender.clone();
        move || {
            let _sent = sender.send(StartupEvent::ReaderClosed);
        }
    }
    pub(super) fn wait(
        self,
        child: &mut Box<dyn Child + Send + Sync>,
        ready_file: &Path,
        screen: &Arc<Terminal>,
        timeout: Duration,
    ) -> Result<()> {
        #[cfg(windows)]
        windows::monitor_child(child.as_ref(), self.sender.clone())?;
        let parent = ready_file
            .parent()
            .context("shell ready path has no parent")?;
        let sender = self.sender.clone();
        let mut watcher = RecommendedWatcher::new(
            move |event: notify::Result<notify::Event>| {
                let _sent = sender.send(StartupEvent::Filesystem(event.map(|_| ())));
            },
            Config::default(),
        )
        .context("failed to create shell startup watcher")?;
        watcher
            .watch(parent, RecursiveMode::NonRecursive)
            .with_context(|| format!("failed to watch shell directory {}", parent.display()))?;
        if startup_result(child, ready_file, screen)?.is_some() {
            return Ok(());
        }
        self.wait_for_event(child, ready_file, screen, timeout)
    }
    fn wait_for_event(
        &self,
        child: &mut Box<dyn Child + Send + Sync>,
        ready_file: &Path,
        screen: &Terminal,
        timeout: Duration,
    ) -> Result<()> {
        let started = Instant::now();
        loop {
            let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
                return startup_timeout(child, ready_file, screen, timeout);
            };
            match self.receiver.recv_timeout(remaining) {
                Ok(StartupEvent::Filesystem(Ok(()))) => {
                    if startup_result(child, ready_file, screen)?.is_some() {
                        return Ok(());
                    }
                }
                Ok(StartupEvent::Filesystem(Err(error))) => {
                    return Err(error).context("shell startup watcher failed");
                }
                Ok(StartupEvent::ReaderClosed) => {
                    return reader_closed_result(child, screen);
                }
                #[cfg(windows)]
                Ok(StartupEvent::ProcessExited(Ok(()))) => {
                    return process_exited_result(child, screen);
                }
                #[cfg(windows)]
                Ok(StartupEvent::ProcessExited(Err(error))) => {
                    return Err(error).context("shell process monitor failed");
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return startup_timeout(child, ready_file, screen, timeout);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    bail!("shell startup event channel disconnected");
                }
            }
        }
    }
}
fn startup_result(
    child: &mut Box<dyn Child + Send + Sync>,
    ready_file: &Path,
    screen: &Terminal,
) -> Result<Option<()>> {
    if let Some(status) = child
        .try_wait()
        .context("failed to inspect shell startup")?
    {
        bail!(
            "shell exited during startup with status {status}; screen: {}",
            startup_screen(screen)
        );
    }
    Ok(ready_file.exists().then_some(()))
}
fn reader_closed_result(child: &mut Box<dyn Child + Send + Sync>, screen: &Terminal) -> Result<()> {
    if let Some(status) = child.try_wait().context("failed to inspect closed shell")? {
        bail!(
            "shell exited during startup with status {status}; screen: {}",
            startup_screen(screen)
        );
    }
    bail!(
        "shell terminal closed during startup; screen: {}",
        startup_screen(screen)
    );
}
#[cfg(windows)]
fn process_exited_result(
    child: &mut Box<dyn Child + Send + Sync>,
    screen: &Terminal,
) -> Result<()> {
    reader_closed_result(child, screen)
}
fn startup_timeout(
    child: &mut Box<dyn Child + Send + Sync>,
    ready_file: &Path,
    screen: &Terminal,
    timeout: Duration,
) -> Result<()> {
    if startup_result(child, ready_file, screen)?.is_some() {
        return Ok(());
    }
    child.kill().context("failed to kill unready shell")?;
    bail!(
        "shell did not report startup readiness within {timeout:?}; screen: {}",
        startup_screen(screen)
    );
}
fn startup_screen(screen: &Terminal) -> String {
    screen.contents().trim().to_owned()
}
enum StartupEvent {
    Filesystem(notify::Result<()>),
    ReaderClosed,
    #[cfg(windows)]
    ProcessExited(Result<()>),
}
