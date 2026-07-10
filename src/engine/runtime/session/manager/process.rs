use anyhow::{Context as _, Result};
use portable_pty::Child;
use std::thread::JoinHandle;
pub(super) fn is_alive(child: &mut (dyn Child + Send + Sync)) -> Result<bool> {
    let status = child.try_wait().context("failed to poll shell child")?;
    Ok(status.is_none())
}
pub(super) fn cleanup(child: &mut (dyn Child + Send + Sync), description: &str) {
    match child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) => {
            if let Err(error) = child.kill() {
                eprintln!("failed to terminate {description}: {error}");
            }
            if let Err(error) = child.wait() {
                eprintln!("failed to wait for {description}: {error}");
            }
        }
        Err(error) => eprintln!("failed to poll {description}: {error}"),
    }
}
pub(super) fn join_reader(reader: JoinHandle<()>, description: &str) {
    if reader.join().is_err() {
        eprintln!("{description} panicked during shell cleanup");
    }
}
