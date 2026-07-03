use crate::runtime::daemon::startup::{StartupReply, READY_ENDPOINT_ENV};
use crate::runtime::daemon::{BootstrapReply, DaemonRequest};
use crate::runtime::protocol::{Payload, Request, Response};
use anyhow::{bail, Context as _, Result};
use core::time::Duration;
use ipc_channel::ipc::{self, IpcOneShotServer, IpcSender};
use std::process::{Command, Stdio};
use std::time::Instant;
const IPC_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
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
                if is_busy_bootstrap_pipe(&error) && start.elapsed() < IPC_CONNECT_TIMEOUT =>
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
        .try_recv_timeout(IPC_CONNECT_TIMEOUT)
        .context("failed to receive daemon IPC channel")?;
    Ok(DaemonClient { sender })
}
fn is_busy_bootstrap_pipe(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}");
    text.contains("-2147024665")
        || text.contains("-2147024360")
        || text.contains("All pipe instances are busy")
        || text.contains("waiting for a process to open the other end of the pipe")
        || text.contains("所有的管道范例都在使用中")
        || text.contains("等候打开管道另一端的进程")
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
            Err(_error) if start.elapsed() < IPC_CONNECT_TIMEOUT => {
                std::thread::yield_now();
            }
            Err(error) => {
                return Err(error).context("daemon instance lock is held but ping failed");
            }
        }
    }
}
fn is_daemon_already_running(error: &anyhow::Error) -> bool {
    format!("{error:#}").contains("daemon is already running")
}
enum StartupEvent {
    Reply(Result<StartupReply>),
    Exit(Result<std::process::ExitStatus>),
}
fn wait_for_daemon_startup(
    startup_server: IpcOneShotServer<StartupReply>,
    mut child: std::process::Child,
) -> Result<()> {
    let (event_sender, event_receiver) = std::sync::mpsc::channel();
    let reply_sender = event_sender.clone();
    let _reply_worker = std::thread::spawn(move || {
        let reply = startup_server
            .accept()
            .map(|(_receiver, reply)| reply)
            .context("failed to receive daemon startup status");
        let _send_result = reply_sender.send(StartupEvent::Reply(reply));
    });
    let _exit_worker = std::thread::spawn(move || {
        let status = child.wait().context("failed to wait for daemon startup");
        let _send_result = event_sender.send(StartupEvent::Exit(status));
    });
    match event_receiver
        .recv()
        .context("failed to receive daemon startup event")?
    {
        StartupEvent::Reply(Ok(StartupReply::Ready)) => Ok(()),
        StartupEvent::Reply(Ok(StartupReply::Failed { message })) => bail!(message),
        StartupEvent::Reply(Err(error)) | StartupEvent::Exit(Err(error)) => Err(error),
        StartupEvent::Exit(Ok(status)) => {
            bail!("daemon exited during startup with status {status}")
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
