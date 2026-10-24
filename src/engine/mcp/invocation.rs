use crate::runtime::{
    client::{self, DaemonClient},
    config::Settings,
    daemon,
    protocol::{Payload, Request},
    session::Manager,
};
use alloc::sync::Arc;
use anyhow::Result;
#[derive(Clone)]
pub(super) struct Backend {
    manager: Option<Arc<Manager>>,
}
impl Backend {
    pub(super) const fn ipc() -> Self {
        Self { manager: None }
    }
    pub(super) const fn direct(manager: Arc<Manager>) -> Self {
        Self {
            manager: Some(manager),
        }
    }
    pub(super) async fn prepare(&self, settings: &Settings) -> Result<()> {
        if self.manager.is_none() {
            client::ensure_daemon(settings).await?;
        }
        Ok(())
    }
    pub(super) async fn execute(&self, settings: &Settings, request: Request) -> Result<Payload> {
        match self.manager.as_ref() {
            None => DaemonClient::connect(settings).await?.call(request).await,
            Some(manager) => daemon::dispatch::execute(manager, request).await,
        }
    }
}
