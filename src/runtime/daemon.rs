use crate::runtime::config::Settings;
use crate::runtime::protocol::{Payload, Request, Response};
use crate::runtime::session::Manager;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
use ipc_channel::TryRecvError;
use ipc_channel::ipc::{self, IpcOneShotServer, IpcSender};
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::thread;
const REQUEST_POLL_INTERVAL: Duration = Duration::from_millis(50);
pub(crate) type BootstrapReply = IpcSender<IpcSender<DaemonRequest>>;
#[derive(Deserialize, Serialize)]
pub(crate) struct DaemonRequest {
    pub(crate) request: Request,
    pub(crate) response: IpcSender<Response>,
}
pub(crate) fn run(settings: Settings) -> Result<()> {
    let service_name = settings.daemon_service_name.clone();
    let (request_sender, request_receiver) =
        ipc::channel::<DaemonRequest>().context("failed to create daemon IPC channel")?;
    let manager = Arc::new(Manager::new(settings)?);
    let (error_sender, error_receiver) = mpsc::channel();
    spawn_bootstrap_server(service_name, request_sender, error_sender);
    loop {
        if let Ok(error) = error_receiver.try_recv() {
            return Err(error);
        }
        match request_receiver.try_recv_timeout(REQUEST_POLL_INTERVAL) {
            Ok(call) => spawn_request_worker(Arc::clone(&manager), call),
            Err(TryRecvError::Empty) => {}
            Err(error) => return Err(error).context("failed to receive IPC request"),
        }
    }
}
fn spawn_bootstrap_server(
    service_name: String,
    request_sender: IpcSender<DaemonRequest>,
    error_sender: mpsc::Sender<anyhow::Error>,
) {
    let _worker = thread::spawn(move || {
        if let Err(error) = serve_bootstrap(&service_name, &request_sender) {
            let _send_result = error_sender.send(error);
        }
    });
}
fn serve_bootstrap(service_name: &str, request_sender: &IpcSender<DaemonRequest>) -> Result<()> {
    loop {
        let (server, endpoint_name) = IpcOneShotServer::<BootstrapReply>::new()
            .context("failed to create IPC bootstrap server")?;
        crate::runtime::ipc_endpoint::publish(service_name, &endpoint_name)?;
        let (_bootstrap_receiver, reply_sender) = server
            .accept()
            .context("failed to accept IPC bootstrap request")?;
        reply_sender
            .send(request_sender.clone())
            .context("failed to send daemon IPC channel")?;
    }
}
fn spawn_request_worker(manager: Arc<Manager>, call: DaemonRequest) {
    let _worker = thread::spawn(move || {
        let response = handle_request(&manager, call.request);
        if let Err(error) = call.response.send(response) {
            eprintln!("failed to send IPC response: {error:#}");
        }
    });
}
fn handle_request(manager: &Arc<Manager>, request: Request) -> Response {
    match dispatch(manager, request) {
        Ok(payload) => Response::Ok { payload },
        Err(error) => Response::Err {
            message: format!("{error:#}"),
        },
    }
}
fn dispatch(manager: &Arc<Manager>, request: Request) -> Result<Payload> {
    match request {
        Request::Ping => Ok(Payload::Pong),
        Request::NewTab {
            starting_directory,
            starting_shell,
        } => {
            let tab_id = manager.new_tab(&starting_directory, starting_shell)?;
            Ok(Payload::TabCreated { tab_id })
        }
        Request::ManualWrite { tab_id, bytes } => {
            manager.manual_write(&tab_id, &bytes)?;
            Ok(Payload::KeyboardWritten)
        }
        Request::SendCommand {
            tab_id,
            command,
            waiting,
        } => {
            let (command_id, end_reason, query) =
                manager.send_command(&tab_id, &command, waiting)?;
            Ok(Payload::CommandAccepted {
                command_id,
                end_reason,
                query,
            })
        }
        Request::Query { id } => Ok(Payload::Query(manager.query(&id)?)),
    }
}
