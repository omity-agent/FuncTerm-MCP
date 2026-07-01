use crate::runtime::protocol::frame::{RequestFrame, ResponseFrame};
use crate::runtime::protocol::wire::{RequestHeader, ResponseHeader};
use crate::runtime::protocol::{Payload, Request, Response};
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use iceoryx2::pending_response::PendingResponse;
use iceoryx2::prelude::*;
use std::process::{Command, Stdio};
use std::time::Instant;
const IPC_TIMEOUT: Duration = Duration::from_secs(15);
const DAEMON_STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const INITIAL_SLICE_LEN: usize = 4096;
type IpcClient = iceoryx2::port::client::Client<
    ipc_threadsafe::Service,
    [u8],
    RequestHeader,
    [u8],
    ResponseHeader,
>;
type IpcPendingResponse =
    PendingResponse<ipc_threadsafe::Service, [u8], RequestHeader, [u8], ResponseHeader>;
pub(crate) struct DaemonClient {
    client: IpcClient,
    request_notifier: crate::runtime::ipc_events::IpcNotifier,
    response_listener: crate::runtime::ipc_events::IpcListener,
}
impl core::fmt::Debug for DaemonClient {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DaemonClient")
    }
}
impl DaemonClient {
    pub(crate) fn connect(service_name: &str) -> Result<Self> {
        let config = crate::runtime::iceoryx::config()?;
        let node = NodeBuilder::new()
            .config(&config)
            .create::<ipc_threadsafe::Service>()
            .context("failed to create iceoryx2 client node")?;
        let service = node
            .service_builder(&service_name.try_into()?)
            .request_response::<[u8], [u8]>()
            .request_user_header::<RequestHeader>()
            .response_user_header::<ResponseHeader>()
            .open_or_create()
            .with_context(|| format!("failed to open iceoryx2 service {service_name}"))?;
        if service.dynamic_config().number_of_servers() == 0 {
            bail!("daemon is not running on iceoryx2 service {service_name}");
        }
        let client = service
            .client_builder()
            .initial_max_slice_len(INITIAL_SLICE_LEN)
            .allocation_strategy(AllocationStrategy::PowerOfTwo)
            .create()
            .context("failed to create iceoryx2 client")?;
        let request_notifier = crate::runtime::ipc_events::request_notifier(&node, service_name)?;
        let response_listener = crate::runtime::ipc_events::response_listener(&node, service_name)?;
        Ok(Self {
            client,
            request_notifier,
            response_listener,
        })
    }
    pub(crate) fn call(&self, request: &Request) -> Result<Payload> {
        let frame = RequestFrame::from_request(request)?;
        let mut uninit_request = self
            .client
            .loan_slice_uninit(frame.payload.len())
            .context("failed to loan iceoryx2 request sample")?;
        *uninit_request.user_header_mut() = frame.header;
        let request_sample = uninit_request.write_from_slice(&frame.payload);
        let pending_response = request_sample
            .send()
            .context("failed to send iceoryx2 request")?;
        self.request_notifier
            .notify()
            .context("failed to notify iceoryx2 request event")?;
        let response = wait_for_response(&self.response_listener, &pending_response)?;
        match response {
            Response::Ok { payload } => Ok(payload),
            Response::Err { message } => bail!(message),
        }
    }
}
pub(crate) fn call(service_name: &str, request: &Request) -> Result<Payload> {
    let client = DaemonClient::connect(service_name)?;
    client.call(request)
}
fn wait_for_response(
    response_listener: &crate::runtime::ipc_events::IpcListener,
    pending_response: &IpcPendingResponse,
) -> Result<Response> {
    let start = Instant::now();
    loop {
        if let Some(response) = pending_response
            .receive()
            .context("failed to receive iceoryx2 response")?
        {
            let frame = ResponseFrame {
                header: *response.user_header(),
                payload: response.payload().to_vec(),
            };
            return frame.into_response();
        }
        let Some(remaining) = IPC_TIMEOUT.checked_sub(start.elapsed()) else {
            bail!("daemon did not respond within {IPC_TIMEOUT:?}");
        };
        response_listener
            .timed_wait_one(remaining)
            .context("failed while waiting for iceoryx2 response event")?;
    }
}
pub(crate) fn ensure_daemon(service_name: &str) -> Result<()> {
    if call(service_name, &Request::Ping).is_ok() {
        return Ok(());
    }
    let config = crate::runtime::iceoryx::config()?;
    let node = NodeBuilder::new()
        .config(&config)
        .create::<ipc_threadsafe::Service>()
        .context("failed to create iceoryx2 daemon startup node")?;
    let ready_listener = crate::runtime::ipc_events::ready_listener(&node, service_name)?;
    let current_exe = std::env::current_exe().context("failed to locate current executable")?;
    let mut command = Command::new(current_exe);
    command
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_detached_flags(&mut command);
    let mut child = command.spawn().context("failed to spawn daemon")?;
    wait_for_daemon_startup_event(&ready_listener)?;
    if let Some(status) = child.try_wait().context("failed to poll daemon startup")? {
        bail!("daemon exited during startup with status {status}");
    }
    if call(service_name, &Request::Ping).is_ok() {
        return Ok(());
    }
    bail!("daemon did not become ready within {DAEMON_STARTUP_TIMEOUT:?}")
}
fn wait_for_daemon_startup_event(
    ready_listener: &crate::runtime::ipc_events::IpcListener,
) -> Result<()> {
    let start = Instant::now();
    loop {
        let Some(remaining) = DAEMON_STARTUP_TIMEOUT.checked_sub(start.elapsed()) else {
            return Ok(());
        };
        if ready_listener
            .timed_wait_one(remaining)
            .context("failed while waiting for iceoryx2 daemon ready event")?
            .is_some()
        {
            return Ok(());
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
