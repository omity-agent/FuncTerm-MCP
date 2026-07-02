use crate::runtime::config::Settings;
use crate::runtime::daemon::startup::StartupReporter;
use crate::runtime::protocol::{Payload, Request, Response};
use crate::runtime::session::Manager;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use ipc_channel::ipc::{self, IpcOneShotServer, IpcReceiver, IpcSender};
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::thread;
pub(crate) mod startup;
pub(crate) type BootstrapReply = IpcSender<IpcSender<DaemonRequest>>;
#[derive(Deserialize, Serialize)]
pub(crate) struct DaemonRequest {
    pub(crate) request: Request,
    pub(crate) response: IpcSender<Response>,
}
pub(crate) fn run(settings: Settings) -> Result<()> {
    let mut startup_reporter = StartupReporter::from_env()?;
    match run_inner(settings, &mut startup_reporter) {
        Ok(()) => Ok(()),
        Err(error) => {
            startup_reporter.failed(&error);
            Err(error)
        }
    }
}
fn run_inner(settings: Settings, startup_reporter: &mut StartupReporter) -> Result<()> {
    let service_name = settings.daemon_service_name.clone();
    let _daemon_instance = crate::runtime::daemon_lock::acquire_instance(&service_name)?;
    let (request_sender, request_receiver) =
        ipc::channel::<DaemonRequest>().context("failed to create daemon IPC channel")?;
    let manager = Arc::new(Manager::new(settings)?);
    let bootstrap_server = publish_bootstrap_server(&service_name)?;
    let (event_sender, event_receiver) = mpsc::channel();
    spawn_bootstrap_server(
        service_name,
        request_sender,
        bootstrap_server,
        event_sender.clone(),
    );
    spawn_request_receiver(request_receiver, event_sender);
    startup_reporter.ready()?;
    loop {
        match event_receiver
            .recv()
            .context("failed to receive daemon runtime event")?
        {
            DaemonEvent::Request(call) => spawn_request_worker(Arc::clone(&manager), call),
            DaemonEvent::Error(error) => return Err(error),
        }
    }
}
enum DaemonEvent {
    Request(DaemonRequest),
    Error(anyhow::Error),
}
fn publish_bootstrap_server(service_name: &str) -> Result<IpcOneShotServer<BootstrapReply>> {
    let (server, endpoint_name) = IpcOneShotServer::<BootstrapReply>::new()
        .context("failed to create IPC bootstrap server")?;
    crate::runtime::ipc_endpoint::publish(service_name, &endpoint_name)?;
    Ok(server)
}
fn spawn_bootstrap_server(
    service_name: String,
    request_sender: IpcSender<DaemonRequest>,
    bootstrap_server: IpcOneShotServer<BootstrapReply>,
    event_sender: mpsc::Sender<DaemonEvent>,
) {
    let _worker = thread::spawn(move || {
        if let Err(error) = serve_bootstrap(&service_name, &request_sender, bootstrap_server) {
            let _send_result = event_sender.send(DaemonEvent::Error(error));
        }
    });
}
fn serve_bootstrap(
    service_name: &str,
    request_sender: &IpcSender<DaemonRequest>,
    mut server: IpcOneShotServer<BootstrapReply>,
) -> Result<()> {
    loop {
        let (_bootstrap_receiver, reply_sender) = server
            .accept()
            .context("failed to accept IPC bootstrap request")?;
        let next_server = publish_bootstrap_server(service_name)?;
        reply_sender
            .send(request_sender.clone())
            .context("failed to send daemon IPC channel")?;
        server = next_server;
    }
}
fn spawn_request_receiver(
    request_receiver: IpcReceiver<DaemonRequest>,
    event_sender: mpsc::Sender<DaemonEvent>,
) {
    let _worker = thread::spawn(move || {
        loop {
            match request_receiver.recv() {
                Ok(call) => {
                    if event_sender.send(DaemonEvent::Request(call)).is_err() {
                        return;
                    }
                }
                Err(error) => {
                    let _send_result = event_sender.send(DaemonEvent::Error(anyhow::anyhow!(
                        "failed to receive IPC request: {error}"
                    )));
                    return;
                }
            }
        }
    });
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
            let (command_id, end_reason, view) =
                manager.send_command(&tab_id, &command, waiting)?;
            Ok(Payload::CommandAccepted {
                command_id,
                end_reason,
                view,
            })
        }
        Request::View { id, waiting } => Ok(Payload::View(manager.view(&id, waiting)?)),
    }
}
