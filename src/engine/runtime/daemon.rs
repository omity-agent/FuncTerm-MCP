use crate::runtime::config::Settings;
use crate::runtime::daemon::report::StartupReporter;
use crate::runtime::protocol::{Payload, Request, Response};
use crate::runtime::session::Manager;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use interprocess::local_socket::prelude::*;
use std::io;
use std::thread;
mod control_signal;
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
    control_signal::enable_ctrl_c_for_descendants()?;
    let service_name = settings.daemon_service_name.clone();
    let _daemon_instance = crate::runtime::daemon_lock::acquire_instance(&service_name)?;
    let manager = Arc::new(Manager::new(settings)?);
    let listener = crate::runtime::transport::listener(&service_name)?;
    startup_reporter.ready()?;
    loop {
        match listener.accept() {
            Ok(stream) => spawn_request_worker(Arc::clone(&manager), stream),
            Err(error) if recoverable_accept_error(&error) => {
                eprintln!("recoverable IPC accept error: {error}");
            }
            Err(error) => return Err(error).context("failed to accept IPC request"),
        }
    }
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
    use windows::Win32::Foundation::{
        ERROR_BROKEN_PIPE, ERROR_NO_DATA, ERROR_OPERATION_ABORTED, ERROR_PIPE_NOT_CONNECTED,
    };
    let Some(code) = error.raw_os_error() else {
        return false;
    };
    let Ok(unsigned_code) = u32::try_from(code) else {
        return false;
    };
    [
        ERROR_BROKEN_PIPE.0,
        ERROR_NO_DATA.0,
        ERROR_OPERATION_ABORTED.0,
        ERROR_PIPE_NOT_CONNECTED.0,
    ]
    .contains(&unsigned_code)
}
#[cfg(not(windows))]
const fn recoverable_platform_accept_error(_error: &io::Error) -> bool {
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
        Request::Close { tab_id } => {
            manager.close(&tab_id)?;
            Ok(Payload::TabClosed { tab_id })
        }
        Request::NewTab {
            starting_directory,
            starting_shell,
            environment,
        } => {
            let tab_id = manager.new_tab(&starting_directory, starting_shell, &environment)?;
            Ok(Payload::TabCreated { tab_id })
        }
        Request::ManualWrite {
            tab_id,
            input,
            waiting,
        } => {
            let view = manager.manual_write(&tab_id, &input, waiting)?;
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
#[path = "../../../tests/unit/runtime/accept_errors.rs"]
mod tests;
