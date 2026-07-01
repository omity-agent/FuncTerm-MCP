use crate::runtime::config::Settings;
use crate::runtime::protocol::frame::{
    BorrowedRequest, decode_request, response_header_len, write_response_payload,
};
use crate::runtime::protocol::wire::{RequestHeader, ResponseHeader};
use crate::runtime::protocol::{Payload, Response};
use crate::runtime::session::Manager;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use iceoryx2::active_request::ActiveRequest;
use iceoryx2::prelude::*;
const INITIAL_SLICE_LEN: usize = 4096;
type IpcActiveRequest =
    ActiveRequest<ipc_threadsafe::Service, [u8], RequestHeader, [u8], ResponseHeader>;
type IpcServer = iceoryx2::port::server::Server<
    ipc_threadsafe::Service,
    [u8],
    RequestHeader,
    [u8],
    ResponseHeader,
>;
pub(crate) fn run(settings: Settings) -> Result<()> {
    let config = crate::runtime::iceoryx::config()?;
    let node = NodeBuilder::new()
        .config(&config)
        .create::<ipc_threadsafe::Service>()
        .context("failed to create iceoryx2 daemon node")?;
    let service = node
        .service_builder(&settings.daemon_service_name.as_str().try_into()?)
        .request_response::<[u8], [u8]>()
        .request_user_header::<RequestHeader>()
        .response_user_header::<ResponseHeader>()
        .open_or_create()
        .with_context(|| {
            format!(
                "failed to open iceoryx2 service {}",
                settings.daemon_service_name
            )
        })?;
    let server = service
        .server_builder()
        .initial_max_slice_len(INITIAL_SLICE_LEN)
        .allocation_strategy(AllocationStrategy::PowerOfTwo)
        .create()
        .context("failed to create iceoryx2 daemon server")?;
    let request_listener =
        crate::runtime::ipc_events::request_listener(&node, &settings.daemon_service_name)?;
    let response_notifier =
        crate::runtime::ipc_events::response_notifier(&node, &settings.daemon_service_name)?;
    let ready_notifier =
        crate::runtime::ipc_events::ready_notifier(&node, &settings.daemon_service_name)?;
    let manager = Arc::new(Manager::new(settings)?);
    ready_notifier
        .notify()
        .context("failed to notify iceoryx2 daemon ready event")?;
    loop {
        while receive_one(&server, &manager, &response_notifier)? {}
        request_listener
            .blocking_wait_one()
            .context("failed while waiting for iceoryx2 request event")?;
    }
}
fn receive_one(
    server: &IpcServer,
    manager: &Arc<Manager>,
    response_notifier: &crate::runtime::ipc_events::IpcNotifier,
) -> Result<bool> {
    if let Some(active_request) = server
        .receive()
        .context("failed to receive iceoryx2 request")?
    {
        handle_request(manager, &active_request)?;
        response_notifier
            .notify()
            .context("failed to notify iceoryx2 response event")?;
        Ok(true)
    } else {
        Ok(false)
    }
}
fn handle_request(manager: &Arc<Manager>, active_request: &IpcActiveRequest) -> Result<()> {
    let response = match decode_request(*active_request.user_header(), active_request.payload()) {
        Ok(request) => match dispatch(manager, &request) {
            Ok(payload) => Response::Ok { payload },
            Err(error) => Response::Err {
                message: format!("{error:#}"),
            },
        },
        Err(error) => Response::Err {
            message: format!("failed to parse request: {error:#}"),
        },
    };
    let (header, payload_len) = response_header_len(&response)?;
    let mut uninit_response = active_request
        .loan_slice_uninit(payload_len)
        .context("failed to loan iceoryx2 response sample")?;
    write_response_payload(&response, uninit_response.payload_mut())?;
    *uninit_response.user_header_mut() = header;
    let response_sample = unsafe { uninit_response.assume_init() };
    response_sample
        .send()
        .context("failed to send iceoryx2 response")
}
#[expect(
    clippy::pattern_type_mismatch,
    reason = "matching borrowed request variants keeps dispatch allocation-free"
)]
fn dispatch(manager: &Arc<Manager>, request: &BorrowedRequest<'_>) -> Result<Payload> {
    match request {
        BorrowedRequest::Ping => Ok(Payload::Pong),
        BorrowedRequest::NewShell { cwd, shell } => {
            let shell_id = manager.new_shell(cwd, *shell)?;
            Ok(Payload::ShellCreated { shell_id })
        }
        BorrowedRequest::WriteKeyboard { shell_id, bytes } => {
            manager.write_keyboard(shell_id, bytes)?;
            Ok(Payload::KeyboardWritten)
        }
        BorrowedRequest::SendCommand {
            shell_id,
            command,
            waiting,
        } => {
            let (command_id, end_reason, query) =
                manager.send_command(shell_id, command, *waiting)?;
            Ok(Payload::CommandAccepted {
                command_id,
                end_reason,
                query,
            })
        }
        BorrowedRequest::Query { id } => Ok(Payload::Query(manager.query(id)?)),
    }
}
