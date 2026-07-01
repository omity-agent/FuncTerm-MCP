use crate::runtime::ipc::{Payload, Request, Response};
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use iceoryx2::pending_response::PendingResponse;
use iceoryx2::prelude::*;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Instant;
const IPC_TIMEOUT: Duration = Duration::from_secs(15);
const DAEMON_STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const IPC_CYCLE: Duration = Duration::from_millis(20);
const INITIAL_SLICE_LEN: usize = 4096;
pub(crate) fn call(service_name: &str, request: &Request) -> Result<Payload> {
    let request_bytes = sonic_rs::to_vec(request).context("failed to serialize request")?;
    let config = crate::runtime::iceoryx::config()?;
    let node = NodeBuilder::new()
        .config(&config)
        .create::<ipc::Service>()
        .context("failed to create iceoryx2 client node")?;
    let service = node
        .service_builder(&service_name.try_into()?)
        .request_response::<[u8], [u8]>()
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
    let uninit_request = client
        .loan_slice_uninit(request_bytes.len())
        .context("failed to loan iceoryx2 request sample")?;
    let request_sample = uninit_request.write_from_slice(&request_bytes);
    let pending_response = request_sample
        .send()
        .context("failed to send iceoryx2 request")?;
    let response_bytes = wait_for_response(&node, &pending_response)?;
    let response =
        sonic_rs::from_slice::<Response>(&response_bytes).context("failed to parse response")?;
    match response {
        Response::Ok { payload } => Ok(payload),
        Response::Err { message } => bail!(message),
    }
}
fn wait_for_response<ServiceType>(
    node: &Node<ipc::Service>,
    pending_response: &PendingResponse<ServiceType, [u8], (), [u8], ()>,
) -> Result<Vec<u8>>
where
    ServiceType: iceoryx2::service::Service,
{
    let start = Instant::now();
    while start.elapsed() < IPC_TIMEOUT {
        if let Some(response) = pending_response
            .receive()
            .context("failed to receive iceoryx2 response")?
        {
            return Ok(response.payload().to_vec());
        }
        node.wait(IPC_CYCLE)
            .context("failed while waiting for iceoryx2 response")?;
    }
    bail!("daemon did not respond within {IPC_TIMEOUT:?}")
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
    while start.elapsed() < DAEMON_STARTUP_TIMEOUT {
        if call(service_name, &Request::Ping).is_ok() {
            return Ok(());
        }
        if let Some(status) = child.try_wait().context("failed to poll daemon startup")? {
            bail!("daemon exited during startup with status {status}");
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("daemon did not become ready within {DAEMON_STARTUP_TIMEOUT:?}")
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
