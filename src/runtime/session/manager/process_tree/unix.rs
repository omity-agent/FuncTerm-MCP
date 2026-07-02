use anyhow::{Context as _, Result, bail};
use portable_pty::Child;
use std::io;
use std::sync::Mutex;
#[derive(Default)]
pub(crate) struct ProcessTree {
    group_id: Mutex<Option<libc::pid_t>>,
}
impl ProcessTree {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self::default())
    }
    pub(crate) fn attach(&self, child: &dyn Child) -> Result<()> {
        let process_id = child
            .process_id()
            .context("shell child does not expose a process id")?;
        let pid = libc::pid_t::try_from(process_id).context("shell child pid exceeds pid_t")?;
        let group_id = unsafe { libc::getpgid(pid) };
        if group_id < 0 {
            return Err(last_error("failed to read shell process group"));
        }
        if group_id != pid {
            bail!(
                "shell child {pid} is in process group {group_id}; expected it to lead an isolated group"
            );
        }
        *self
            .group_id
            .lock()
            .map_err(|error| anyhow::anyhow!("process group mutex poisoned: {error}"))? =
            Some(group_id);
        Ok(())
    }
    pub(crate) fn terminate(&self) -> Result<()> {
        let group_id = *self
            .group_id
            .lock()
            .map_err(|error| anyhow::anyhow!("process group mutex poisoned: {error}"))?;
        let Some(group_id) = group_id else {
            return Ok(());
        };
        terminate_group(group_id, libc::SIGHUP)?;
        terminate_group(group_id, libc::SIGTERM)?;
        terminate_group(group_id, libc::SIGKILL)?;
        Ok(())
    }
}
fn terminate_group(group_id: libc::pid_t, signal: libc::c_int) -> Result<()> {
    if group_id <= 1 {
        bail!("refusing to signal unsafe process group {group_id}");
    }
    let target = group_id
        .checked_neg()
        .context("failed to construct process group signal target")?;
    let sent = unsafe { libc::kill(target, signal) };
    if sent == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(anyhow::anyhow!(
        "failed to send signal {signal} to process group {group_id}: {error}"
    ))
}
fn last_error(message: &str) -> anyhow::Error {
    let error = io::Error::last_os_error();
    anyhow::anyhow!("{message}: {error}")
}
#[cfg(test)]
mod tests {
    use super::ProcessTree;
    use core::time::Duration;
    use std::os::unix::process::CommandExt as _;
    use std::process::Command;
    use std::time::Instant;
    #[test]
    fn terminate_kills_attached_process_group() {
        let mut command = Command::new("sh");
        command.args(["-c", "trap '' TERM; sleep 30 & wait"]);
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command.spawn().unwrap();
        let started_at = Instant::now();
        let tree = ProcessTree::new().unwrap();
        tree.attach(&child).unwrap();
        tree.terminate().unwrap();
        child.wait().unwrap();
        assert!(started_at.elapsed() < Duration::from_secs(5));
    }
}
