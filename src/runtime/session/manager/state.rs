use super::{Manager, ShellSession};
use crate::runtime::session::records::{CommandRecord, read_done};
use crate::runtime::session::support::lock_mutex;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};
use uuid::Uuid;
impl Manager {
    pub(super) fn next_id(&self) -> Result<String> {
        loop {
            let id = Uuid::new_v4().simple().to_string();
            let short_id = id
                .get(..12)
                .context("generated UUID was shorter than 12 characters")?
                .to_owned();
            if !self.id_exists(&short_id)? {
                return Ok(short_id);
            }
        }
    }
    pub(super) fn shell_cwd(shell: &ShellSession) -> Result<PathBuf> {
        Ok(lock_mutex(&shell.cwd, "cwd")?.clone())
    }
    pub(super) fn command_fallback_cwd(&self, record: &CommandRecord) -> Result<PathBuf> {
        if let Some(shell) = self.find_shell(&record.shell_id)? {
            return Self::shell_cwd(&shell);
        }
        Ok(record.initial_cwd.clone())
    }
    pub(super) fn update_shell_cwd(
        shell: &Arc<ShellSession>,
        record: &CommandRecord,
    ) -> Result<()> {
        if let Some(done) = read_done(&record.done)? {
            *lock_mutex(&shell.cwd, "cwd")? = PathBuf::from(done.cwd);
        }
        Ok(())
    }
    fn id_exists(&self, id: &str) -> Result<bool> {
        let shell_exists = lock_mutex(&self.shells, "shell")?.contains_key(id);
        let command_exists = lock_mutex(&self.commands, "command")?.contains_key(id);
        Ok(shell_exists || command_exists)
    }
}
pub(super) fn path_text(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .with_context(|| format!("cwd is not valid UTF-8: {}", path.display()))
}
