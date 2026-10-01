use crate::runtime::config::IpcSettings;
use crate::runtime::protocol::{Request, Response};
use anyhow::{Context as _, Result};
use futures_util::{SinkExt as _, StreamExt as _};
use interprocess::ConnectWaitMode;
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ListenerOptions};
use serde::{Serialize, de::DeserializeOwned};
use tokio_serde::{Framed as SerdeFramed, formats::MessagePack};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
type Transport = Framed<LocalSocketStream, LengthDelimitedCodec>;
pub(crate) type ClientConnection = Connection<Response, Request>;
pub(crate) type ServerConnection = Connection<Request, Response>;
pub(crate) struct Connection<Incoming, Outgoing> {
    framed: SerdeFramed<Transport, Incoming, Outgoing, MessagePack<Incoming, Outgoing>>,
}
impl<Incoming, Outgoing> Connection<Incoming, Outgoing>
where
    Incoming: DeserializeOwned + Unpin,
    Outgoing: Serialize + Unpin,
{
    pub(crate) fn new(stream: LocalSocketStream, settings: &IpcSettings) -> Self {
        let transport = LengthDelimitedCodec::builder()
            .max_frame_length(settings.max_frame_bytes)
            .new_framed(stream);
        Self {
            framed: SerdeFramed::new(transport, MessagePack::default()),
        }
    }
    pub(crate) async fn send(&mut self, value: Outgoing) -> Result<()> {
        self.framed
            .send(value)
            .await
            .context("failed to send IPC frame")
    }
    pub(crate) async fn receive(&mut self) -> Result<Incoming> {
        self.receive_or_eof()
            .await?
            .context("IPC stream ended before a message was received")
    }
    pub(crate) async fn receive_or_eof(&mut self) -> Result<Option<Incoming>> {
        self.framed
            .next()
            .await
            .transpose()
            .context("failed to receive IPC frame")
    }
}
pub(crate) fn listener(service_name: &str) -> Result<LocalSocketListener> {
    let socket_name = socket_name(service_name);
    let name = socket_name
        .as_str()
        .to_ns_name::<GenericNamespaced>()
        .context("failed to create daemon socket name")?;
    ListenerOptions::new()
        .name(name)
        .try_overwrite(true)
        .create_tokio()
        .context("failed to listen on daemon IPC socket")
}
pub(crate) async fn connect(
    service_name: &str,
    settings: &IpcSettings,
) -> Result<ClientConnection> {
    let socket_name = socket_name(service_name);
    let name = socket_name
        .as_str()
        .to_ns_name::<GenericNamespaced>()
        .context("failed to create daemon socket name")?;
    let stream = interprocess::local_socket::ConnectOptions::new()
        .name(name)
        .wait_mode(ConnectWaitMode::Timeout(settings.setup_timeout()?))
        .connect_tokio()
        .await
        .with_context(|| format!("daemon is not running on IPC service {service_name}"))?;
    Ok(Connection::new(stream, settings))
}
pub(crate) fn lock_name(service_name: &str, kind: &str) -> String {
    format!("functerm-{kind}-{}", service_digest(service_name))
}
fn socket_name(service_name: &str) -> String {
    format!("functerm-ipc-{}", service_digest(service_name))
}
fn service_digest(service_name: &str) -> String {
    blake3::hash(service_name.as_bytes()).to_hex().to_string()
}
