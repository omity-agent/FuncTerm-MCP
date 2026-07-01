use crate::runtime::session::records::wait_for_path;
use crate::runtime::session::support::lock_mutex;
use crate::shell::ShellStartup;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use portable_pty::{Child, CommandBuilder};
use std::path::Path;
use std::sync::Mutex;
pub(super) fn apply_startup(command: &mut CommandBuilder, startup: ShellStartup) {
    command.args(startup.args);
}
pub(super) fn wait_for_shell_startup(
    child: &mut Box<dyn Child + Send + Sync>,
    ready_file: &Path,
    screen: &Arc<Mutex<vt100::Parser>>,
) -> Result<()> {
    if let Some(status) = child.try_wait().context("failed to poll shell startup")? {
        bail!(
            "shell exited during startup with status {status}; screen: {}",
            startup_screen(screen)?
        );
    }
    if wait_for_path(ready_file, Duration::from_secs(5))? {
        return Ok(());
    }
    if let Some(status) = child.try_wait().context("failed to poll shell startup")? {
        bail!(
            "shell exited during startup with status {status}; screen: {}",
            startup_screen(screen)?
        );
    }
    child.kill().context("failed to kill unready shell")?;
    bail!(
        "shell did not report startup readiness within 5s; screen: {}",
        startup_screen(screen)?
    );
}
fn startup_screen(screen: &Arc<Mutex<vt100::Parser>>) -> Result<String> {
    let contents = lock_mutex(screen, "screen")?.screen().contents();
    Ok(contents.trim().to_owned())
}
