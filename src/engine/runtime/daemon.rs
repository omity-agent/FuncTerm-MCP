use crate::runtime::config::Settings;
use crate::runtime::daemon::report::StartupReporter;
use crate::runtime::protocol::{Payload, Request, Response};
use crate::runtime::session::Manager;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use interprocess::local_socket::prelude::*;
use std::io;
use std::sync::mpsc;
use std::thread;
pub(crate) mod report;
pub(crate) fn run(settings: Settings) -> Result<()> {
    let mut startup_reporter = StartupReporter::from_env();
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
    let manager = Arc::new(Manager::new(settings)?);
    let listener = crate::runtime::transport::listener(&service_name)?;
    let (event_sender, event_receiver) = mpsc::channel();
    spawn_request_receiver(listener, event_sender);
    startup_reporter.ready()?;
    loop {
        match event_receiver
            .recv()
            .context("failed to receive daemon runtime event")?
        {
            DaemonEvent::Request(stream) => spawn_request_worker(Arc::clone(&manager), stream),
            DaemonEvent::Error(error) => return Err(error),
        }
    }
}
enum DaemonEvent {
    Request(LocalSocketStream),
    Error(anyhow::Error),
}
fn spawn_request_receiver(listener: LocalSocketListener, event_sender: mpsc::Sender<DaemonEvent>) {
    let _worker = thread::spawn(move || {
        loop {
            match listener.accept() {
                Ok(stream) => {
                    if event_sender.send(DaemonEvent::Request(stream)).is_err() {
                        return;
                    }
                }
                Err(error) if recoverable_accept_error(&error) => {
                    eprintln!("recoverable IPC accept error: {error}");
                }
                Err(error) => {
                    let _send_result = event_sender.send(DaemonEvent::Error(anyhow::anyhow!(
                        "failed to accept IPC request: {error}"
                    )));
                    return;
                }
            }
        }
    });
}
fn recoverable_accept_error(error: &io::Error) -> bool {
    if matches!(
        error.kind(),
        io::ErrorKind::Interrupted
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::BrokenPipe
    ) {
        return true;
    }
    recoverable_platform_accept_error(error)
}
#[cfg(windows)]
fn recoverable_platform_accept_error(error: &io::Error) -> bool {
    use windows_sys::Win32::Foundation::{
        ERROR_BROKEN_PIPE, ERROR_NO_DATA, ERROR_OPERATION_ABORTED, ERROR_PIPE_NOT_CONNECTED,
    };
    let Some(code) = error.raw_os_error() else {
        return false;
    };
    matches!(
        u32::try_from(code),
        Ok(ERROR_BROKEN_PIPE | ERROR_NO_DATA | ERROR_OPERATION_ABORTED | ERROR_PIPE_NOT_CONNECTED)
    )
}
#[cfg(not(windows))]
fn recoverable_platform_accept_error(_error: &io::Error) -> bool {
    false
}
fn spawn_request_worker(manager: Arc<Manager>, mut stream: LocalSocketStream) {
    let _worker = thread::spawn(move || {
        loop {
            let request = match crate::runtime::transport::read_frame_or_eof::<Request>(&mut stream)
            {
                Ok(Some(request)) => request,
                Ok(None) => return,
                Err(error) => {
                    eprintln!("failed to read IPC request: {error:#}");
                    return;
                }
            };
            let response = handle_request(&manager, request);
            if let Err(error) = crate::runtime::transport::write_frame(&mut stream, &response) {
                eprintln!("failed to send IPC response: {error:#}");
                return;
            }
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
            environment,
        } => {
            let tab_id = manager.new_tab(&starting_directory, starting_shell, &environment)?;
            Ok(Payload::TabCreated { tab_id })
        }
        Request::ManualWrite { tab_id, bytes } => {
            let view = manager.manual_write(&tab_id, &bytes)?;
            Ok(Payload::KeyboardWritten { view })
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
#[cfg(test)]
mod tests {
    use std::io;
    #[test]
    fn interrupted_accept_error_is_recoverable() {
        let error = io::Error::from(io::ErrorKind::Interrupted);
        assert!(super::recoverable_accept_error(&error));
    }
    #[test]
    fn permission_accept_error_is_fatal() {
        let error = io::Error::from(io::ErrorKind::PermissionDenied);
        assert!(!super::recoverable_accept_error(&error));
    }
    #[cfg(windows)]
    #[test]
    fn abandoned_named_pipe_accept_error_is_recoverable() {
        let code = i32::try_from(windows_sys::Win32::Foundation::ERROR_OPERATION_ABORTED).unwrap();
        let error = io::Error::from_raw_os_error(code);
        assert!(super::recoverable_accept_error(&error));
    }
}
