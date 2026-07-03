use crate::runtime::daemon::startup::{READY_ENDPOINT_ENV, StartupReply};
use crate::runtime::daemon::{BootstrapReply, DaemonRequest};
use crate::runtime::protocol::{Payload, Request, Response};
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use ipc_channel::ipc::{self, IpcOneShotServer, IpcSender};
use std::process::{Command, Stdio};
use std::time::Instant;
const IPC_SETUP_TIMEOUT: Duration = Duration::from_secs(15);
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
        connect_to_endpoint(service_name)
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
            .recv()
            .context("failed to receive IPC response")?;
        match response {
            Response::Ok { payload } => Ok(payload),
            Response::Err { message } => bail!(message),
        }
    }
}
fn connect_to_endpoint(service_name: &str) -> Result<DaemonClient> {
    let start = Instant::now();
    loop {
        let endpoint_name = crate::runtime::ipc_endpoint::read(service_name)?;
        match try_connect_to_endpoint(service_name, endpoint_name) {
            Ok(client) => return Ok(client),
            Err(error)
                if crate::runtime::ipc_failure::is_retryable_bootstrap_error(&error)
                    && start.elapsed() < IPC_SETUP_TIMEOUT =>
            {
                std::thread::yield_now();
            }
            Err(error) => return Err(error),
        }
    }
}
fn try_connect_to_endpoint(service_name: &str, endpoint_name: String) -> Result<DaemonClient> {
    let (reply_sender, reply_receiver) =
        ipc::channel::<IpcSender<DaemonRequest>>().context("failed to create IPC reply")?;
    let bootstrap = IpcSender::<BootstrapReply>::connect(endpoint_name)
        .with_context(|| format!("daemon is not running on IPC service {service_name}"))?;
    bootstrap
        .send(reply_sender)
        .context("failed to request daemon IPC channel")?;
    let sender = reply_receiver
        .try_recv_timeout(IPC_SETUP_TIMEOUT)
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
    let _startup_lock = crate::runtime::daemon_lock::acquire_startup(service_name)?;
    if call(service_name, &Request::Ping).is_ok() {
        return Ok(());
    }
    match spawn_daemon() {
        Ok(()) => Ok(()),
        Err(error) if is_daemon_already_running(&error) => wait_for_existing_daemon(service_name),
        Err(error) => Err(error),
    }
}
fn spawn_daemon() -> Result<()> {
    let (startup_server, startup_endpoint) =
        IpcOneShotServer::<StartupReply>::new().context("failed to create startup IPC server")?;
    let current_exe = std::env::current_exe().context("failed to locate current executable")?;
    let mut command = Command::new(current_exe);
    command
        .arg("daemon")
        .env(READY_ENDPOINT_ENV, startup_endpoint)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_detached_flags(&mut command);
    let child = command.spawn().context("failed to spawn daemon")?;
    wait_for_daemon_startup(startup_server, child)
}
fn wait_for_existing_daemon(service_name: &str) -> Result<()> {
    let start = Instant::now();
    loop {
        match call(service_name, &Request::Ping) {
            Ok(Payload::Pong) => return Ok(()),
            Ok(_payload) => bail!("daemon returned an unexpected response to ping"),
            Err(_error) if start.elapsed() < IPC_SETUP_TIMEOUT => {
                std::thread::yield_now();
            }
            Err(error) => {
                return Err(error).context("daemon instance lock is held but ping failed");
            }
        }
    }
}
fn is_daemon_already_running(error: &anyhow::Error) -> bool {
    crate::runtime::daemon_lock::already_running_service_name(error).is_some()
}
fn wait_for_daemon_startup(
    startup_server: IpcOneShotServer<StartupReply>,
    mut child: std::process::Child,
) -> Result<()> {
    let (reply_sender, reply_receiver) = std::sync::mpsc::channel();
    let _reply_worker = std::thread::spawn(move || {
        let reply = startup_server
            .accept()
            .map(|(_receiver, reply)| reply)
            .context("failed to receive daemon startup status");
        let _send_result = reply_sender.send(reply);
    });
    match reply_receiver.recv_timeout(IPC_SETUP_TIMEOUT) {
        Ok(Ok(StartupReply::Ready)) => Ok(()),
        Ok(Ok(StartupReply::AlreadyRunning { service_name })) => {
            Err(crate::runtime::daemon_lock::DaemonAlreadyRunning::new(service_name).into())
        }
        Ok(Ok(StartupReply::Failed { message })) => bail!(message),
        Ok(Err(error)) => Err(error),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => fail_daemon_startup_timeout(&mut child),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            bail!("daemon startup reporter disconnected")
        }
    }
}
fn fail_daemon_startup_timeout(child: &mut std::process::Child) -> Result<()> {
    if let Some(status) = child
        .try_wait()
        .context("failed to poll daemon startup status")?
    {
        bail!("daemon exited during startup with status {status}");
    }
    match child.kill() {
        Ok(()) => {
            let _status = child
                .wait()
                .context("failed to wait for daemon after startup timeout")?;
            bail!("daemon startup timed out after {IPC_SETUP_TIMEOUT:?}");
        }
        Err(kill_error) => {
            if let Some(status) = child
                .try_wait()
                .context("failed to poll daemon startup status after kill failed")?
            {
                bail!("daemon exited during startup with status {status}");
            }
            Err(kill_error).context("failed to kill daemon after startup timeout")
        }
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
