use crate::runtime::daemon::{BootstrapReply, DaemonRequest};
use crate::runtime::protocol::{Payload, Request, Response};
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use ipc_channel::ipc::{self, IpcSender};
use std::process::{Command, Stdio};
use std::time::Instant;
const IPC_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_RETRY_TIMEOUT: Duration = Duration::from_secs(2);
const DAEMON_STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const STARTUP_RETRY_DELAY: Duration = Duration::from_millis(50);
pub(crate) struct DaemonClient {
    sender: IpcSender<DaemonRequest>,
}
impl core::fmt::Debug for DaemonClient {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DaemonClient")
    }
}
impl DaemonClient {
    pub(crate) fn connect(service_name: &str) -> Result<Self> {
        let start = Instant::now();
        loop {
            let endpoint_name = crate::runtime::ipc_endpoint::read(service_name)?;
            match connect_to_endpoint(service_name, endpoint_name) {
                Ok(client) => return Ok(client),
                Err(error) if start.elapsed() >= CONNECT_RETRY_TIMEOUT => return Err(error),
                Err(_error) => {}
            }
            std::thread::park_timeout(STARTUP_RETRY_DELAY);
        }
    }
    pub(crate) fn call(&self, request: &Request) -> Result<Payload> {
        let (response_sender, response_receiver) =
            ipc::channel::<Response>().context("failed to create IPC response channel")?;
        self.sender
            .send(DaemonRequest {
                request: request.clone(),
                response: response_sender,
            })
            .context("failed to send IPC request")?;
        let response = response_receiver
            .try_recv_timeout(IPC_TIMEOUT)
            .context("failed to receive IPC response")?;
        match response {
            Response::Ok { payload } => Ok(payload),
            Response::Err { message } => bail!(message),
        }
    }
}
fn connect_to_endpoint(service_name: &str, endpoint_name: String) -> Result<DaemonClient> {
    let (reply_sender, reply_receiver) =
        ipc::channel::<IpcSender<DaemonRequest>>().context("failed to create IPC reply")?;
    let bootstrap = IpcSender::<BootstrapReply>::connect(endpoint_name)
        .with_context(|| format!("daemon is not running on IPC service {service_name}"))?;
    bootstrap
        .send(reply_sender)
        .context("failed to request daemon IPC channel")?;
    let sender = reply_receiver
        .try_recv_timeout(IPC_TIMEOUT)
        .context("failed to receive daemon IPC channel")?;
    Ok(DaemonClient { sender })
}
pub(crate) fn call(service_name: &str, request: &Request) -> Result<Payload> {
    let client = DaemonClient::connect(service_name)?;
    client.call(request)
}
pub(crate) fn ensure_daemon(service_name: &str) -> Result<()> {
    if call(service_name, &Request::Ping).is_ok() {
        return Ok(());
    }
    let current_exe = std::env::current_exe().context("failed to locate current executable")?;
    let mut command = Command::new(current_exe);
    command
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_detached_flags(&mut command);
    let mut child = command.spawn().context("failed to spawn daemon")?;
    let start = Instant::now();
    loop {
        if call(service_name, &Request::Ping).is_ok() {
            return Ok(());
        }
        if let Some(status) = child.try_wait().context("failed to poll daemon startup")? {
            bail!("daemon exited during startup with status {status}");
        }
        if start.elapsed() >= DAEMON_STARTUP_TIMEOUT {
            bail!("daemon did not become ready within {DAEMON_STARTUP_TIMEOUT:?}");
        }
        std::thread::park_timeout(STARTUP_RETRY_DELAY);
    }
}
#[cfg(windows)]
fn apply_detached_flags(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}
#[cfg(not(windows))]
fn apply_detached_flags(_command: &mut Command) {}
