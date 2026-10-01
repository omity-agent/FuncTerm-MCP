use crate::runtime::config::Settings;
use crate::runtime::daemon::report::StartupReporter;
use crate::runtime::protocol::Response;
use crate::runtime::session::Manager;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use interprocess::local_socket::tokio::prelude::*;
use std::io;
use tokio::task::JoinSet;
mod control_signal;
mod dispatch;
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
    let manager = Arc::new(
        tokio::task::spawn_blocking(move || Manager::new(settings))
            .await
            .context("daemon initialization worker failed")??,
    );
    let listener = crate::runtime::transport::listener(&service_name)?;
    startup_reporter.ready()?;
    let mut workers = JoinSet::new();
    loop {
        tokio::select! { accepted = listener . accept () => match accepted { Ok (stream) => { let connection = crate :: runtime :: transport :: ServerConnection :: new (stream , & ipc) ; workers . spawn (serve_connection (Arc :: clone (& manager) , connection)) ; } Err (error) if recoverable_accept_error (& error) => { eprintln ! ("recoverable IPC accept error: {error}") ; } Err (error) => return Err (error) . context ("failed to accept IPC request") , } , Some (result) = workers . join_next () , if ! workers . is_empty () => { match result { Ok (Ok (())) => { } Ok (Err (error)) => eprintln ! ("IPC connection failed: {error:#}") , Err (error) => eprintln ! ("IPC connection task failed: {error}") , } } }
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
async fn serve_connection(
    manager: Arc<Manager>,
    mut connection: crate::runtime::transport::ServerConnection,
) -> Result<()> {
    while let Some(request) = connection.receive_or_eof().await? {
        let response = match dispatch::execute(&manager, request).await {
            Ok(payload) => Response::Ok { payload },
            Err(error) => Response::Err {
                message: format!("{error:#}"),
            },
        };
        connection.send(response).await?;
    }
    Ok(())
}
