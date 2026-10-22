use crate::runtime::config::Settings;
use crate::runtime::daemon::report::StartupReporter;
use crate::runtime::protocol::{Payload, Request, Response};
use crate::runtime::session::Manager;
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
pub(crate) async fn run(settings: Settings) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_new(&settings.diagnostics_filter)
                .context("invalid diagnostics_filter")?,
        )
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize daemon diagnostics: {error}"))?;
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
    let _daemon_instance = crate::runtime::daemon_lock::acquire_instance(&service_name)?;
    let ipc = settings.ipc.clone();
    let http_settings = settings.clone();
    let manager = Arc::new(
        tokio::task::spawn_blocking(move || Manager::new(settings))
            .await
            .context("daemon initialization worker failed")??,
    );
    let listener = crate::runtime::transport::listener(&service_name, &ipc)?;
    let mut http = crate::mcp::HttpServer::bind(http_settings, Arc::clone(&manager)).await?;
    startup_reporter.ready(http.address())?;
    let mut workers = JoinSet::new();
    let (shutdown, mut shutdown_requests) = mpsc::channel(1);
    let shutdown_reason = loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(stream) => {
                    let connection = ServerConnection::new(stream, &ipc);
                    workers.spawn(serve_connection(
                        Arc::clone(&manager),
                        connection,
                        shutdown.clone(),
                    ));
                }
                Err(error) if recoverable_accept_error(&error) => {
                    eprintln!("recoverable IPC accept error: {error}");
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
        tokio::join!(http.stop(ipc.shutdown_timeout()?), manager.shutdown(),);
    if let Err(error) = http_stopped.as_ref() {
        tracing::error!(%error, "MCP HTTP shutdown failed");
    }
    if let Err(error) = stopped.as_ref() {
        tracing::error!(%error, "terminal session shutdown failed");
    }
    let mut shutdown_connection = shutdown_reason?;
    let completion = http_stopped.and(stopped);
    let response = response_from(completion.as_ref().map(|&()| Payload::ShutdownComplete));
    let sent = shutdown_connection.send(response).await;
    completion?;
    sent.context("failed to acknowledge daemon shutdown")
}
fn report_worker(result: Result<Result<()>, tokio::task::JoinError>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => eprintln!("IPC connection failed: {error:#}"),
        Err(error) if error.is_cancelled() => {}
        Err(error) => eprintln!("IPC connection task failed: {error}"),
    }
}
fn response_from<E>(result: Result<Payload, E>) -> Response
where
    E: core::fmt::Display,
{
    match result {
        Ok(payload) => Response::Ok { payload },
        Err(error) => Response::Err {
            message: format!("{error:#}"),
        },
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
    manager: Arc<Manager>,
    mut connection: ServerConnection,
    shutdown: mpsc::Sender<ServerConnection>,
) -> Result<()> {
    while let Some(request) = connection.receive_or_eof().await? {
        if matches!(request, Request::Shutdown) {
            shutdown
                .send(connection)
                .await
                .map_err(|error| anyhow::anyhow!("failed to request daemon shutdown: {error}"))?;
            return Ok(());
        }
        let response = response_from(dispatch::execute(&manager, request).await);
        connection.send(response).await?;
    }
    Ok(())
}
