use crate::runtime::protocol::{Payload, Request, Response};
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
const IPC_SETUP_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) struct DaemonClient {
    service_name: String,
}
impl core::fmt::Debug for DaemonClient {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DaemonClient")
    }
}
impl DaemonClient {
    pub(crate) fn connect(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_owned(),
        }
    }
    pub(crate) fn call(&self, request: &Request) -> Result<Payload> {
        call(&self.service_name, request)
    }
}
pub(crate) fn call(service_name: &str, request: &Request) -> Result<Payload> {
    let mut stream = crate::runtime::transport::connect(service_name, IPC_SETUP_TIMEOUT)?;
    crate::runtime::transport::write_frame(&mut stream, request)?;
    let response = crate::runtime::transport::read_frame::<Response>(&mut stream)?;
    match response {
        Response::Ok { payload } => Ok(payload),
        Response::Err { message } => bail!(message),
    }
}
pub(crate) fn ensure_daemon(service_name: &str) -> Result<()> {
    if call(service_name, &Request::Ping).is_ok() {
        return Ok(());
    }
    let _startup_lock = crate::runtime::daemon_lock::acquire_startup(service_name)?;
    if call(service_name, &Request::Ping).is_ok() {
        return Ok(());
    }
    match super::daemon_spawn::spawn_daemon(service_name, IPC_SETUP_TIMEOUT) {
        Ok(()) => wait_for_existing_daemon(service_name),
        Err(error) if is_daemon_already_running(&error) => wait_for_existing_daemon(service_name),
        Err(error) => Err(error),
    }
}
pub(crate) fn run_daemon_launcher() -> Result<()> {
    let settings = crate::runtime::config::load()?;
    super::daemon_spawn::run_launcher(&settings.daemon_service_name)
}
fn wait_for_existing_daemon(service_name: &str) -> Result<()> {
    match call(service_name, &Request::Ping) {
        Ok(Payload::Pong) => Ok(()),
        Ok(_payload) => bail!("daemon returned an unexpected response to ping"),
        Err(error) => Err(error).context("daemon instance lock is held but ping failed"),
    }
}
fn is_daemon_already_running(error: &anyhow::Error) -> bool {
    crate::runtime::daemon_lock::already_running_service_name(error).is_some()
}
