use anyhow::{Context as _, Result};
use kill_tree::{Config, blocking::kill_tree_with_config};
use portable_pty::Child;
use std::sync::Mutex;
#[derive(Default)]
pub(super) struct ProcessTree {
    process_id: Mutex<Option<u32>>,
}
impl ProcessTree {
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn attach(&self, child: &dyn Child) -> Result<()> {
        let process_id = child
            .process_id()
            .context("shell child does not expose a process id")?;
        *self
            .process_id
            .lock()
            .map_err(|error| anyhow::anyhow!("process tree mutex poisoned: {error}"))? =
            Some(process_id);
        Ok(())
    }
    pub(super) fn terminate(&self) -> Result<()> {
        let stored_process_id = *self
            .process_id
            .lock()
            .map_err(|error| anyhow::anyhow!("process tree mutex poisoned: {error}"))?;
        let Some(process_id) = stored_process_id else {
            return Ok(());
        };
        let config = Config {
            signal: "SIGKILL".to_owned(),
            ..Default::default()
        };
        kill_tree_with_config(process_id, &config)
            .with_context(|| format!("failed to terminate shell process tree {process_id}"))?;
        Ok(())
    }
}
