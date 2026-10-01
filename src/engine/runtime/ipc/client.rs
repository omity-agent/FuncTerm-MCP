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
    let service = settings.daemon_service_name.clone();
    let _startup_lock =
        tokio::task::spawn_blocking(move || crate::runtime::daemon_lock::acquire_startup(&service))
            .await
            .context("daemon startup lock worker failed")??;
    if probe(settings).await? {
        return Ok(());
    }
    let service_name = settings.daemon_service_name.clone();
    let timeout = settings.ipc.setup_timeout()?;
    let spawned = tokio::task::spawn_blocking(move || {
        super::daemon_spawn::spawn_daemon(&service_name, timeout)
    })
    .await
    .context("daemon launch worker failed")?;
    if let Err(error) = spawned
        && !is_daemon_already_running(&error)
    {
        return Err(error);
    }
    if probe(settings).await? {
        return Ok(());
    }
    bail!("daemon startup completed without accepting IPC connections")
}
pub(crate) fn run_daemon_launcher() -> Result<()> {
    let settings = crate::runtime::config::load()?;
    super::daemon_spawn::run_launcher(&settings.daemon_service_name)
}
async fn probe(settings: &Settings) -> Result<bool> {
    let mut client = match DaemonClient::connect(settings).await {
        Ok(client) => client,
        Err(error) if endpoint_absent(&error) => return Ok(false),
        Err(error) => return Err(error),
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
fn is_daemon_already_running(error: &anyhow::Error) -> bool {
    crate::runtime::daemon_lock::already_running_service_name(error).is_some()
}
