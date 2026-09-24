use anyhow::{Context as _, Result};
use kill_tree::{Config, blocking::kill_tree_with_config};
use parking_lot::Mutex;
use portable_pty::Child;
use std::thread::JoinHandle;
#[derive(Default)]
pub(in crate::engine::runtime::session::manager) struct ProcessTree {
    process_id: Mutex<Option<u32>>,
}
impl ProcessTree {
    pub(in crate::engine::runtime::session::manager) fn new() -> Self {
        Self::default()
    }
    pub(in crate::engine::runtime::session::manager) fn attach(
        &self,
        child: &dyn Child,
    ) -> Result<()> {
        let process_id = child
            .process_id()
            .context("shell child does not expose a process id")?;
        *self.process_id.lock() = Some(process_id);
        Ok(())
    }
    pub(in crate::engine::runtime::session::manager) fn terminate(&self) -> Result<()> {
        let mut stored_process_id = self.process_id.lock();
        let Some(process_id) = *stored_process_id else {
            return Ok(());
        };
        let config = Config {
            signal: "SIGKILL".to_owned(),
            ..Default::default()
        };
        kill_tree_with_config(process_id, &config)
            .with_context(|| format!("failed to terminate shell process tree {process_id}"))?;
        *stored_process_id = None;
        drop(stored_process_id);
        Ok(())
    }
}
pub(in crate::engine::runtime::session::manager) fn is_alive(
    child: &mut (dyn Child + Send + Sync),
) -> Result<bool> {
    let status = child.try_wait().context("failed to poll shell child")?;
    Ok(status.is_none())
}
pub(in crate::engine::runtime::session::manager) fn cleanup(
    child: &mut (dyn Child + Send + Sync),
    description: &str,
) {
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
pub(in crate::engine::runtime::session::manager) fn join_reader(
    reader: JoinHandle<()>,
    description: &str,
) {
    if reader.join().is_err() {
        eprintln!("{description} panicked during shell cleanup");
    }
}
