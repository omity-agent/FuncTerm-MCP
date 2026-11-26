mod directory;
mod shutdown;
mod snapshot;
mod tab_view;
use self::snapshot::TabSnapshot;
use super::BlockingExecutor;
use super::shell_session::ShellSession;
use crate::runtime::protocol::{OutputTokenizer, RequestRejected, ShellView, ViewResult};
use alloc::sync::Arc;
use anyhow::Result;
pub(super) use directory::TabDirectory;
use parking_lot::Mutex;
use tokio_util::task::TaskTracker;
pub(super) struct Tab {
    id: String,
    operation: Arc<tokio::sync::Mutex<()>>,
    pub(super) blocking: BlockingExecutor,
    pub(super) tokenizer: Arc<OutputTokenizer>,
    pub(super) supervisors: TaskTracker,
    state: Mutex<TabState>,
}
struct TabState {
    session: Option<Arc<ShellSession>>,
    snapshot: TabSnapshot,
}
impl Tab {
    pub(super) fn new(
        id: String,
        session: Arc<ShellSession>,
        blocking: BlockingExecutor,
        tokenizer: Arc<OutputTokenizer>,
        supervisors: TaskTracker,
    ) -> Result<Self> {
        let snapshot = TabSnapshot::from_session(&session)?;
        Ok(Self {
            id,
            operation: Arc::default(),
            blocking,
            tokenizer,
            supervisors,
            state: Mutex::new(TabState {
                session: Some(session),
                snapshot,
            }),
        })
    }
    pub(super) fn id(&self) -> &str {
        &self.id
    }
    pub(super) async fn with_operation<T>(
        self: &Arc<Self>,
        operation: impl FnOnce(&Arc<Self>) -> Result<T> + Send + 'static,
    ) -> Result<T>
    where
        T: Send + 'static,
    {
        let guard = Arc::clone(&self.operation).lock_owned().await;
        let tab = Arc::clone(self);
        self.blocking
            .run(move || {
                let _guard = guard;
                operation(&tab)
            })
            .await
    }
    pub(super) fn closed_tab(&self) -> RequestRejected {
        RequestRejected::new(format!(
            "\"{}\" exists in the records, but the tab has been closed. Did you run an exit command within it?",
            self.id
        ))
    }
    pub(super) fn remember(&self, session: &ShellSession) -> Result<TabSnapshot> {
        let snapshot = TabSnapshot::from_session(session)?;
        self.state.lock().snapshot = snapshot.clone();
        Ok(snapshot)
    }
    pub(super) fn record_closure(&self, session: &ShellSession) -> Result<()> {
        let snapshot = TabSnapshot::from_session(session);
        let mut state = self.state.lock();
        state.session = None;
        state.snapshot = snapshot?;
        drop(state);
        Ok(())
    }
    pub(super) fn snapshot_view(&self) -> ViewResult {
        self.state.lock().snapshot.clone().into_view(false)
    }
    pub(super) fn snapshot_shell_view(&self) -> ShellView {
        self.state.lock().snapshot.clone().shell_view(false)
    }
    pub(super) fn optional_session(&self) -> Option<Arc<ShellSession>> {
        self.state.lock().session.clone()
    }
    pub(super) fn observe_session<T>(&self, observe: impl FnOnce(&ShellSession) -> T) -> Option<T> {
        self.state.lock().session.as_deref().map(observe)
    }
    pub(super) fn has_session(&self) -> bool {
        self.state.lock().session.is_some()
    }
}
