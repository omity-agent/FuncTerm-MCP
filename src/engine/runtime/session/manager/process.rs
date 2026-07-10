use anyhow::{Context as _, Result};
use rmux_pty::PtyChild;
use std::thread::JoinHandle;
pub(super) fn is_alive(child: &mut PtyChild) -> Result<bool> {
    let status = child.try_wait().context("failed to poll shell child")?;
    if status.is_some() {
        close_pseudoconsole(child);
    }
    Ok(status.is_none())
}
pub(super) fn cleanup(child: &mut PtyChild, description: &str) {
    match child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) => {
            if let Err(error) = child.terminate_forcefully() {
                eprintln!("failed to terminate {description}: {error}");
            }
            if let Err(error) = child.wait() {
                eprintln!("failed to wait for {description}: {error}");
            }
        }
        Err(error) => eprintln!("failed to poll {description}: {error}"),
    }
    close_pseudoconsole(child);
}
pub(super) fn join_reader(reader: JoinHandle<()>, description: &str) {
    if reader.join().is_err() {
        eprintln!("{description} panicked during shell cleanup");
    }
}
#[cfg(windows)]
fn close_pseudoconsole(child: &PtyChild) {
    child.close_pseudoconsole();
}
#[cfg(not(windows))]
fn close_pseudoconsole(_child: &PtyChild) {}
