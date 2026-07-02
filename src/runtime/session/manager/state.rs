use super::{Manager, ShellSession};
use crate::runtime::session::records::{CommandRecord, read_done};
use crate::runtime::session::support::lock_mutex;
use crate::shell::shims;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};
const ID_LENGTH: usize = 12;
const ID_ALPHABET: [char; 36] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];
impl Manager {
    pub(super) fn next_tab_id(&self) -> Result<String> {
        self.next_id("tab-")
    }
    pub(super) fn next_command_id(&self) -> Result<String> {
        self.next_id("command-")
    }
    fn next_id(&self, prefix: &str) -> Result<String> {
        loop {
            let id = format!("{prefix}{}", random_id_suffix());
            if !self.id_exists(&id)? {
                return Ok(id);
            }
        }
    }
    pub(super) fn shell_cwd(shell: &ShellSession) -> Result<PathBuf> {
        Ok(lock_mutex(&shell.cwd, "cwd")?.clone())
    }
    pub(super) fn command_fallback_cwd(&self, record: &CommandRecord) -> Result<PathBuf> {
        if let Some(shell) = self.find_shell(&record.tab_id)? {
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
    pub(super) fn refresh_shell_choice(shell: &ShellSession) -> Result<()> {
        if let Some(choice) = shims::read_active_shell(&shell.active_shell_file)? {
            *lock_mutex(&shell.choice, "choice")? = choice;
        }
        Ok(())
    }
    fn id_exists(&self, id: &str) -> Result<bool> {
        let shell_exists = lock_mutex(&self.shells, "shell")?.contains_key(id);
        let command_exists = lock_mutex(&self.commands, "command")?.contains_key(id);
        let tab_id_exists = self.generated_tab_id_exists(id)?;
        Ok(shell_exists || command_exists || tab_id_exists)
    }
}
fn random_id_suffix() -> String {
    nanoid::nanoid!(ID_LENGTH, &ID_ALPHABET)
}
pub(super) fn path_text(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .with_context(|| format!("cwd is not valid UTF-8: {}", path.display()))
}
