use crate::runtime::{
    client::DaemonClient,
    config::Settings,
    daemon::dispatch::Dispatcher,
    protocol::{Payload, Request},
};
use alloc::sync::Arc;
use anyhow::Result;
use parking_lot::Mutex;
#[derive(Clone)]
pub(super) struct Backend {
    dispatcher: Option<Arc<Dispatcher>>,
}
pub(super) struct PreparedBackend<'dispatcher> {
    dispatcher: Option<&'dispatcher Dispatcher>,
    connection: Mutex<Option<DaemonClient>>,
}
impl Backend {
    pub(super) const fn ipc() -> Self {
        Self { dispatcher: None }
    }
    pub(super) const fn direct(dispatcher: Arc<Dispatcher>) -> Self {
        Self {
            dispatcher: Some(dispatcher),
        }
    }
    pub(super) async fn prepare(&self, settings: &Settings) -> Result<PreparedBackend<'_>> {
        let connection = if self.dispatcher.is_none() {
            Some(DaemonClient::connect_or_start(settings).await?)
        } else {
            None
        };
        Ok(PreparedBackend {
            dispatcher: self.dispatcher.as_deref(),
            connection: Mutex::new(connection),
        })
    }
}
impl PreparedBackend<'_> {
    pub(super) async fn execute(&self, settings: &Settings, request: Request) -> Result<Payload> {
        if let Some(dispatcher) = self.dispatcher {
            return dispatcher.execute(request).await;
        }
        let prepared = self.connection.lock().take();
        let mut connection = match prepared {
            Some(client) => client,
            None => DaemonClient::connect(settings).await?,
        };
        connection.call(request).await
    }
}
