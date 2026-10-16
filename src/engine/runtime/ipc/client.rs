use super::endpoint::ClientConnection;
use crate::runtime::config::Settings;
use crate::runtime::protocol::{Payload, Request, RequestKind, Response};
use anyhow::{Context as _, Result, bail};
use std::io;
pub(crate) struct DaemonClient {
    stream: ClientConnection,
}
impl core::fmt::Debug for DaemonClient {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DaemonClient")
    }
}
impl DaemonClient {
    pub(crate) async fn connect(settings: &Settings) -> Result<Self> {
        let stream =
            crate::runtime::transport::connect(&settings.daemon_service_name, &settings.ipc)
                .await?;
        Ok(Self { stream })
    }
    pub(crate) async fn connect_if_running(settings: &Settings) -> Result<Option<Self>> {
        match Self::connect(settings).await {
            Ok(client) => Ok(Some(client)),
            Err(error) if endpoint_absent(&error) => Ok(None),
            Err(error) => Err(error),
        }
    }
    pub(crate) async fn call(&mut self, request: Request) -> Result<Payload> {
        let kind = RequestKind::from(&request);
        self.stream.send(request).await?;
        match self.stream.receive().await? {
            Response::Ok { payload } => payload.ensure_matches(kind),
            Response::Err { message } => bail!(message),
        }
    }
}
pub(crate) async fn ensure_daemon(settings: &Settings) -> Result<()> {
    if probe(settings).await? {
        return Ok(());
    }
    super::service_control::start(settings).await?;
    Ok(())
}
pub(crate) fn run_daemon_launcher() -> Result<()> {
    let settings = crate::runtime::config::load()?;
    super::daemon_spawn::run_launcher(&settings.daemon_service_name)
}
pub(super) async fn probe(settings: &Settings) -> Result<bool> {
    let Some(mut client) = DaemonClient::connect_if_running(settings).await? else {
        return Ok(false);
    };
    tokio::time::timeout(settings.ipc.setup_timeout()?, client.call(Request::Ping))
        .await
        .context("daemon did not respond to ping before the IPC setup timeout")??;
    Ok(true)
}
fn endpoint_absent(error: &anyhow::Error) -> bool {
    error.downcast_ref::<io::Error>().is_some_and(|source| {
        matches!(
            source.kind(),
            io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
        )
    })
}
