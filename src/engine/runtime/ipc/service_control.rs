use super::{client, daemon_spawn, lock};
use crate::runtime::config::Settings;
use crate::runtime::protocol::Request;
use anyhow::{Context as _, Result, bail};

#[derive(Clone, Copy)]
enum Operation {
    Start,
    Stop,
    Restart,
}

pub(crate) async fn start(settings: &Settings) -> Result<bool> {
    serialized(settings, Operation::Start).await
}

pub(crate) async fn stop(settings: &Settings) -> Result<bool> {
    serialized(settings, Operation::Stop).await
}

pub(crate) async fn restart(settings: &Settings) -> Result<()> {
    serialized(settings, Operation::Restart).await?;
    Ok(())
}

async fn serialized(settings: &Settings, operation: Operation) -> Result<bool> {
    let configuration = settings.clone();
    tokio::task::spawn_blocking(move || {
        // Windows named mutexes are acquired and released on the same worker thread.
        let _lifecycle_lock = lock::acquire_startup(&configuration.daemon_service_name)?;
        tokio::runtime::Handle::current().block_on(async {
            match operation {
                Operation::Start => start_locked(&configuration).await,
                Operation::Stop => stop_locked(&configuration).await,
                Operation::Restart => {
                    stop_locked(&configuration).await?;
                    start_locked(&configuration).await
                }
            }
        })
    })
    .await
    .context("daemon lifecycle worker failed")?
}

async fn start_locked(settings: &Settings) -> Result<bool> {
    if client::probe(settings).await? {
        return Ok(false);
    }
    let service_name = settings.daemon_service_name.clone();
    let timeout = settings.ipc.setup_timeout()?;
    let spawned =
        tokio::task::spawn_blocking(move || daemon_spawn::spawn_daemon(&service_name, timeout))
            .await
            .context("daemon launch worker failed")?;
    if let Err(error) = spawned
        && lock::already_running_service_name(&error).is_none()
    {
        return Err(error);
    }
    if client::probe(settings).await? {
        return Ok(true);
    }
    bail!("daemon startup completed without accepting IPC connections")
}

async fn stop_locked(settings: &Settings) -> Result<bool> {
    let Some(mut connection) = client::DaemonClient::connect_if_running(settings).await? else {
        return Ok(false);
    };
    tokio::time::timeout(
        settings.ipc.shutdown_timeout()?,
        connection.call(Request::Shutdown),
    )
    .await
    .context("daemon did not shut down before the IPC shutdown timeout")??;
    lock::wait_for_exit(&settings.daemon_service_name)?;
    Ok(true)
}
