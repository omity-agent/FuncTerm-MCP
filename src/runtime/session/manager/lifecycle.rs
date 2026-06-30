use super::ShellSession;
use crate::runtime::session::support::lock_mutex;
use anyhow::{Result, bail};
#[expect(
    clippy::missing_trait_methods,
    reason = "Drop only needs the regular destructor for this type"
)]
impl Drop for ShellSession {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            match child.try_wait() {
                Ok(Some(_)) => {}
                Ok(None) => {
                    if let Err(error) = child.kill() {
                        eprintln!("failed to kill shell child during cleanup: {error}");
                    }
                    if let Err(error) = child.wait() {
                        eprintln!("failed to wait shell child during cleanup: {error}");
                    }
                }
                Err(error) => eprintln!("failed to poll shell child during cleanup: {error}"),
            }
        }
    }
}
pub(super) fn reserve_shell(shell: &ShellSession, command_id: &str) -> Result<()> {
    {
        let mut busy = lock_mutex(&shell.busy, "busy")?;
        if let Some(existing_id) = busy.as_deref() {
            bail!("shell is busy with command {existing_id}");
        }
        *busy = Some(command_id.to_owned());
    }
    Ok(())
}
pub(super) fn release_shell(shell: &ShellSession, command_id: &str) -> Result<()> {
    {
        let mut busy = lock_mutex(&shell.busy, "busy")?;
        if busy.as_deref() == Some(command_id) {
            *busy = None;
        }
    }
    Ok(())
}
