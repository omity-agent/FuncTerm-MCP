use crate::runtime::config::Settings;
use crate::runtime::daemon::dispatch::Dispatcher;
use crate::runtime::daemon::report::StartupReporter;
use crate::runtime::protocol::time_consumption::{self, ProcessingTime};
use crate::runtime::protocol::{Payload, Request, RequestKind, RequestRejected, Response};
use crate::runtime::transport::ServerConnection;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use interprocess::local_socket::tokio::prelude::*;
use std::io;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
mod control_signal;
pub(crate) mod dispatch;
pub(crate) mod report;
pub(crate) mod resource_alerts;
pub(crate) async fn run(settings: Settings) -> Result<()> {
    crate::engine::init_diagnostics(&settings.diagnostics_filter)?;
    let mut startup_reporter = StartupReporter::from_env();
    match run_inner(settings, &mut startup_reporter).await {
        Ok(()) => Ok(()),
        Err(error) => {
            startup_reporter.failed(&error);
            Err(error)
        }
    }
}
async fn run_inner(settings: Settings, startup_reporter: &mut StartupReporter) -> Result<()> {
    control_signal::enable_ctrl_c_for_descendants()?;
    let service_name = settings.daemon_service_name.clone();
    let _daemon_instance = crate::runtime::daemon_lock::acquire_instance(&settings)?;
    let ipc = settings.ipc.clone();
    let measure_processing = settings.benchmark.processing_timing;
    let http_settings = settings.clone();
    let handler = tokio::task::spawn_blocking(move || Dispatcher::new(settings))
        .await
        .context("daemon initialization worker failed")??;
    let dispatcher = Arc::new(handler);
    let listener = crate::runtime::transport::listener(&service_name, &ipc)?;
    let mut http = crate::mcp::HttpServer::bind(http_settings, Arc::clone(&dispatcher)).await?;
    startup_reporter.ready(http.address())?;
    let mut workers = JoinSet::new();
    let (shutdown, mut shutdown_requests) = mpsc::channel(1);
    let shutdown_reason = loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(stream) => {
                    let connection = ServerConnection::new(stream, &ipc);
                    workers.spawn(serve_connection(
                        Arc::clone(&dispatcher),
                        connection,
                        shutdown.clone(),
                        measure_processing,
                    ));
                }
                Err(error) if recoverable_accept_error(&error) => {
                    tracing::warn!(%error, "recoverable IPC accept error");
                }
                Err(error) => break Err(error).context("failed to accept IPC request"),
            },
            Some(connection) = shutdown_requests.recv() => break Ok(connection),
            error = http.failure() => break Err(error),
            Some(result) = workers.join_next(), if !workers.is_empty() => {
                report_worker(result);
            }
        }
    };
    drop(listener);
    drop(shutdown_requests);
    workers.abort_all();
    while let Some(result) = workers.join_next().await {
        report_worker(result);
    }
    let (http_stopped, stopped) =
        tokio::join!(http.stop(ipc.shutdown_timeout()?), dispatcher.shutdown());
    if let Err(error) = http_stopped.as_ref() {
        tracing::error!("MCP HTTP shutdown failed");
        tracing::debug!(
            error = format!("{error:#}"),
            "MCP HTTP shutdown failure details"
        );
    }
    if let Err(error) = stopped.as_ref() {
        tracing::error!("terminal session shutdown failed");
        tracing::debug!(
            error = format!("{error:#}"),
            "terminal session shutdown failure details"
        );
    }
    let mut shutdown_connection = shutdown_reason?;
    let completion = http_stopped.and(stopped);
    let response = response_from(
        completion.as_ref().map(|&()| Payload::ShutdownComplete),
        None,
    );
    let sent = shutdown_connection.send(response).await;
    completion?;
    sent.context("failed to acknowledge daemon shutdown")
}
fn report_worker(result: Result<Result<()>, tokio::task::JoinError>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            tracing::error!("IPC connection failed");
            tracing::debug!(
                error = format!("{error:#}"),
                "IPC connection failure details"
            );
        }
        Err(error) if error.is_cancelled() => {}
        Err(error) => {
            tracing::error!("IPC connection task failed");
            tracing::debug!(%error, "IPC connection task failure details");
        }
    }
}
fn response_from(
    result: Result<Payload, impl core::borrow::Borrow<anyhow::Error>>,
    processing: Option<ProcessingTime>,
) -> Response {
    match result {
        Ok(payload) => Response::Ok {
            payload,
            processing,
        },
        Err(error) => {
            let failure = error.borrow();
            if let Some(rejection) = failure.downcast_ref::<RequestRejected>() {
                return Response::Rejected {
                    message: rejection.to_string(),
                    processing,
                };
            }
            tracing::error!("daemon request failed");
            tracing::debug!(
                error = format!("{failure:#}"),
                "daemon request failure details"
            );
            Response::Failed { processing }
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
    use windows::Win32::{
        ERROR_BROKEN_PIPE, ERROR_NO_DATA, ERROR_OPERATION_ABORTED, ERROR_PIPE_NOT_CONNECTED,
    };
    let Some(code) = error.raw_os_error() else {
        return false;
    };
    [
        ERROR_BROKEN_PIPE,
        ERROR_NO_DATA,
        ERROR_OPERATION_ABORTED,
        ERROR_PIPE_NOT_CONNECTED,
    ]
    .contains(&code)
}
#[cfg(not(windows))]
const fn recoverable_platform_accept_error(_error: &io::Error) -> bool {
    false
}
async fn serve_connection(
    dispatcher: Arc<Dispatcher>,
    mut connection: ServerConnection,
    shutdown: mpsc::Sender<ServerConnection>,
    measure_processing: bool,
) -> Result<()> {
    while let Some(request) = connection.receive_or_eof().await? {
        if matches!(request, Request::Shutdown) {
            shutdown
                .send(connection)
                .await
                .map_err(|error| anyhow::anyhow!("failed to request daemon shutdown: {error}"))?;
            return Ok(());
        }
        let kind = RequestKind::from(&request);
        tracing::trace!(%kind, "received IPC request");
        let (result, processing) =
            time_consumption::capture(measure_processing, dispatcher.execute(request)).await;
        let response = response_from(result, processing);
        tracing::trace!(%kind, "sending IPC response");
        connection.send(response).await?;
        tracing::trace!(%kind, "sent IPC response");
    }
    Ok(())
}
