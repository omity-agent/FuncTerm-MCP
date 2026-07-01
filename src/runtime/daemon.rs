use crate::runtime::config::Settings;
use crate::runtime::ipc::{Payload, Request, Response};
use crate::runtime::session::Manager;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use base64_turbo::STANDARD;
use core::time::Duration;
use iceoryx2::active_request::ActiveRequest;
use iceoryx2::prelude::*;
const IPC_CYCLE: Duration = Duration::from_millis(20);
const INITIAL_SLICE_LEN: usize = 4096;
pub(crate) fn run(settings: Settings) -> Result<()> {
    let config = crate::runtime::iceoryx::config()?;
    let node = NodeBuilder::new()
        .config(&config)
        .create::<ipc::Service>()
        .context("failed to create iceoryx2 daemon node")?;
    let service = node
        .service_builder(&settings.daemon_service_name.as_str().try_into()?)
        .request_response::<[u8], [u8]>()
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
    let manager = Arc::new(Manager::new(settings)?);
    while node.wait(IPC_CYCLE).is_ok() {
        while let Some(active_request) = server
            .receive()
            .context("failed to receive iceoryx2 request")?
        {
            handle_request(&manager, &active_request)?;
        }
    }
    Ok(())
}
fn handle_request<ServiceType>(
    manager: &Arc<Manager>,
    active_request: &ActiveRequest<ServiceType, [u8], (), [u8], ()>,
) -> Result<()>
where
    ServiceType: iceoryx2::service::Service,
{
    let response = match sonic_rs::from_slice::<Request>(active_request.payload()) {
        Ok(request) => match dispatch(manager, request) {
            Ok(payload) => Response::Ok { payload },
            Err(error) => Response::Err {
                message: format!("{error:#}"),
            },
        },
        Err(error) => Response::Err {
            message: format!("failed to parse request: {error:#}"),
        },
    };
    let response_bytes = sonic_rs::to_vec(&response).context("failed to serialize response")?;
    let uninit_response = active_request
        .loan_slice_uninit(response_bytes.len())
        .context("failed to loan iceoryx2 response sample")?;
    let response_sample = uninit_response.write_from_slice(&response_bytes);
    response_sample
        .send()
        .context("failed to send iceoryx2 response")
}
fn dispatch(manager: &Arc<Manager>, request: Request) -> Result<Payload> {
    match request {
        Request::Ping => Ok(Payload::Pong),
        Request::NewShell { cwd, shell } => {
            let shell_id = manager.new_shell(&cwd, shell)?;
            Ok(Payload::ShellCreated { shell_id })
        }
        Request::WriteKeyboard {
            shell_id,
            bytes_base64,
        } => {
            let bytes = STANDARD
                .decode(bytes_base64)
                .context("failed to decode keyboard bytes")?;
            manager.write_keyboard(&shell_id, &bytes)?;
            Ok(Payload::KeyboardWritten)
        }
        Request::SendCommand {
            shell_id,
            command,
            wait_ms,
        } => {
            let (command_id, end_reason, query) =
                manager.send_command(&shell_id, &command, wait_ms)?;
            Ok(Payload::CommandAccepted {
                command_id,
                end_reason,
                query,
            })
        }
        Request::Query { id } => Ok(Payload::Query(manager.query(&id)?)),
    }
}
