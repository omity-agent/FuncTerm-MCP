use crate::runtime::{
    client::DaemonClient,
    config::Settings,
    daemon,
    protocol::{Payload, Request},
    session::Manager,
};
use alloc::sync::Arc;
use anyhow::Result;
use parking_lot::Mutex;
#[derive(Clone)]
pub(super) struct Backend {
    manager: Option<Arc<Manager>>,
}
pub(super) struct PreparedBackend<'manager> {
    manager: Option<&'manager Arc<Manager>>,
    connection: Mutex<Option<DaemonClient>>,
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
    pub(super) async fn prepare(&self, settings: &Settings) -> Result<PreparedBackend<'_>> {
        let connection = if self.manager.is_none() {
            Some(DaemonClient::connect_or_start(settings).await?)
        } else {
            None
        };
        Ok(PreparedBackend {
            manager: self.manager.as_ref(),
            connection: Mutex::new(connection),
        })
    }
}
impl PreparedBackend<'_> {
    pub(super) async fn execute(&self, settings: &Settings, request: Request) -> Result<Payload> {
        if let Some(manager) = self.manager {
            return daemon::dispatch::execute(manager, request).await;
        }
        let prepared = self.connection.lock().take();
        let mut connection = match prepared {
            Some(client) => client,
            None => DaemonClient::connect(settings).await?,
        };
        connection.call(request).await
    }
}
