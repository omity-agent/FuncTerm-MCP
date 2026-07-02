use super::process_tree;
use crate::runtime::session::support::lock_mutex;
use crate::shell::{ShellChoice, shims};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use portable_pty::{Child, SlavePty};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
pub(super) struct ShellSession {
    pub(super) choice: Mutex<ShellChoice>,
    pub(super) cwd: Mutex<PathBuf>,
    pub(super) writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub(super) screen: Arc<Mutex<vt100::Parser>>,
    pub(super) last_command: Mutex<Option<String>>,
    pub(super) busy: Mutex<Option<String>>,
    pub(super) command_root: PathBuf,
    pub(super) active_shell_file: PathBuf,
    pub(super) process_tree: process_tree::ProcessTree,
    pub(super) child: Mutex<Box<dyn Child + Send + Sync>>,
    pub(super) _slave: Mutex<Box<dyn SlavePty + Send>>,
}
impl ShellSession {
    pub(super) fn cwd(&self) -> Result<PathBuf> {
        Ok(lock_mutex(&self.cwd, "cwd")?.clone())
    }
    pub(super) fn set_cwd(&self, cwd: PathBuf) -> Result<()> {
        *lock_mutex(&self.cwd, "cwd")? = cwd;
        Ok(())
    }
    pub(super) fn current_choice(&self) -> Result<ShellChoice> {
        Ok(*lock_mutex(&self.choice, "choice")?)
    }
    pub(super) fn refresh_choice(&self) -> Result<()> {
        if let Some(choice) = shims::read_active_shell(&self.active_shell_file)? {
            *lock_mutex(&self.choice, "choice")? = choice;
        }
        Ok(())
    }
    pub(super) fn is_alive(&self) -> Result<bool> {
        let status = lock_mutex(&self.child, "child")?
            .try_wait()
            .context("failed to poll shell child")?;
        Ok(status.is_none())
    }
}
impl Drop for ShellSession {
    fn drop(&mut self) {
        if let Err(error) = self.process_tree.terminate() {
            eprintln!("failed to terminate shell process tree during cleanup: {error}");
        }
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
