use anyhow::{Context as _, Result};
use core::time::Duration;
use interprocess::ConnectWaitMode;
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ListenerOptions};
use serde::{Serialize, de::DeserializeOwned};
use std::io::{Read as _, Write as _};
const FRAME_LIMIT: usize = 64 * 1024 * 1024;
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
pub(crate) fn connect(service_name: &str, timeout: Duration) -> Result<LocalSocketStream> {
    let socket_name = socket_name(service_name);
    let name = socket_name
        .as_str()
        .to_ns_name::<GenericNamespaced>()
        .context("failed to create daemon socket name")?;
    interprocess::local_socket::ConnectOptions::new()
        .name(name)
        .wait_mode(ConnectWaitMode::Timeout(timeout))
        .connect_sync()
        .with_context(|| format!("daemon is not running on IPC service {service_name}"))
}
pub(crate) fn write_frame<T>(stream: &mut LocalSocketStream, value: &T) -> Result<()>
where
    T: Serialize,
{
    let body = sonic_rs::to_string(value).context("failed to serialize IPC frame")?;
    let body_len = u32::try_from(body.len()).context("IPC frame is too large to encode")?;
    stream
        .write_all(&body_len.to_be_bytes())
        .context("failed to write IPC frame length")?;
    stream
        .write_all(body.as_bytes())
        .context("failed to write IPC frame body")?;
    stream.flush().context("failed to flush IPC frame")
}
pub(crate) fn read_frame<T>(stream: &mut LocalSocketStream) -> Result<T>
where
    T: DeserializeOwned,
{
    let mut length = [0_u8; 4];
    stream
        .read_exact(&mut length)
        .context("failed to read IPC frame length")?;
    let body_len = usize::try_from(u32::from_be_bytes(length))
        .context("IPC frame length does not fit usize")?;
    anyhow::ensure!(
        body_len <= FRAME_LIMIT,
        "IPC frame length {body_len} exceeds {FRAME_LIMIT}"
    );
    let mut body = vec![0_u8; body_len];
    stream
        .read_exact(&mut body)
        .context("failed to read IPC frame body")?;
    let text = core::str::from_utf8(&body).context("IPC frame is not valid UTF-8")?;
    sonic_rs::from_str(text).context("failed to parse IPC frame")
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
