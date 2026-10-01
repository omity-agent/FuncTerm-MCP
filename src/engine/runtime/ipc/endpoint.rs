use anyhow::{Context as _, Result};
use core::time::Duration;
use interprocess::ConnectWaitMode;
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ListenerOptions};
use serde::{Serialize, de::DeserializeOwned};
use std::io::BufReader;
pub(crate) struct Connection {
    reader: BufReader<LocalSocketStream>,
}
impl Connection {
    pub(crate) fn new(stream: LocalSocketStream) -> Self {
        Self {
            reader: BufReader::new(stream),
        }
    }
    pub(crate) fn send<T>(&mut self, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        super::codec::write(self.reader.get_mut(), value)
    }
    pub(crate) fn receive<T>(&mut self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        self.receive_or_eof()?
            .context("IPC stream ended before a message was received")
    }
    pub(crate) fn receive_or_eof<T>(&mut self) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        super::codec::read_or_eof(&mut self.reader)
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
        .create_sync()
        .context("failed to listen on daemon IPC socket")
}
pub(crate) fn connect(service_name: &str, timeout: Duration) -> Result<Connection> {
    let socket_name = socket_name(service_name);
    let name = socket_name
        .as_str()
        .to_ns_name::<GenericNamespaced>()
        .context("failed to create daemon socket name")?;
    interprocess::local_socket::ConnectOptions::new()
        .name(name)
        .wait_mode(ConnectWaitMode::Timeout(timeout))
        .connect_sync()
        .map(Connection::new)
        .with_context(|| format!("daemon is not running on IPC service {service_name}"))
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
