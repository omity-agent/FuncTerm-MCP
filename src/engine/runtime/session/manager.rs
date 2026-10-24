mod command;
mod launcher;
mod shell_session;
mod tab;
use crate::runtime::config::Settings;
use crate::runtime::protocol::{EnvironmentSnapshot, OutputTokenizer};
use crate::shell::ShellChoice;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail, ensure};
use parking_lot::RwLock;
use shell_session::process;
use std::path::{Path, PathBuf};
use tokio::sync::Semaphore;
use tokio_util::task::TaskTracker;
pub(crate) struct Manager {
    launcher: launcher::ShellLauncher,
    tabs: tab::TabDirectory,
    blocking: BlockingExecutor,
    tokenizer: Arc<OutputTokenizer>,
    accepting_tabs: RwLock<bool>,
    supervisors: TaskTracker,
    shutdown_timeout: core::time::Duration,
}
impl Manager {
    pub(crate) async fn close(&self, tab_id: &str) -> Result<()> {
        self.tabs.close(tab_id).await
    }
    pub(crate) fn new(settings: Settings) -> Result<Self> {
        let blocking = BlockingExecutor::new(settings.ipc.blocking_concurrency);
        let tokenizer = Arc::new(OutputTokenizer::new(&settings.output_token_encoding)?);
        let shutdown_timeout = settings.ipc.shutdown_timeout()?;
        Ok(Self {
            launcher: launcher::ShellLauncher::new(settings)?,
            tabs: tab::TabDirectory::default(),
            blocking,
            tokenizer,
            accepting_tabs: RwLock::new(true),
            supervisors: TaskTracker::new(),
            shutdown_timeout,
        })
    }
    pub(crate) async fn shutdown(self: &Arc<Self>) -> Result<()> {
        let manager = Arc::clone(self);
        self.blocking
            .run(move || {
                *manager.accepting_tabs.write() = false;
                Ok(())
            })
            .await?;
        let closed = self.tabs.close_all().await;
        self.supervisors.close();
        tokio::time::timeout(self.shutdown_timeout, self.supervisors.wait())
            .await
            .context("command supervision did not finish before daemon shutdown timeout")?;
        closed
    }
    pub(crate) async fn new_tab(
        self: &Arc<Self>,
        starting_directory: PathBuf,
        starting_shell: ShellChoice,
        load_profile: bool,
        elevated: bool,
        environment: EnvironmentSnapshot,
    ) -> Result<String> {
        let manager = Arc::clone(self);
        self.blocking
            .run(move || {
                manager.launch_tab(
                    &starting_directory,
                    starting_shell,
                    load_profile,
                    elevated,
                    &environment,
                )
            })
            .await
    }
    fn launch_tab(
        &self,
        starting_directory: &Path,
        starting_shell: ShellChoice,
        load_profile: bool,
        elevated: bool,
        environment: &EnvironmentSnapshot,
    ) -> Result<String> {
        let accepting = self.accepting_tabs.read();
        ensure!(*accepting, "daemon is shutting down");
        if !starting_directory.is_dir() {
            bail!(
                "starting_directory does not exist or is not a directory: {}",
                starting_directory.display()
            );
        }
        let tab_id = self.tabs.next_tab_id();
        let session = self.launcher.launch(
            &tab_id,
            starting_directory,
            starting_shell,
            load_profile,
            elevated,
            environment,
        )?;
        self.tabs.insert(tab::Tab::new(
            tab_id.clone(),
            session,
            self.blocking.clone(),
            Arc::clone(&self.tokenizer),
            self.supervisors.clone(),
        )?);
        drop(accepting);
        Ok(tab_id)
    }
}
#[derive(Clone)]
pub(super) struct BlockingExecutor {
    permits: Arc<Semaphore>,
}
impl BlockingExecutor {
    fn new(concurrency: usize) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(concurrency)),
        }
    }
    pub(super) async fn run<T, F>(&self, operation: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T> + Send + 'static,
    {
        let permit = Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .context("blocking executor is closed")?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            operation()
        })
        .await
        .context("blocking operation worker failed")?
    }
}
