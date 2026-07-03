use crate::runtime::daemon::startup::{READY_STDOUT_ENV, StartupReply};
use crate::runtime::protocol::{Payload, Request, Response};
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use std::io::BufRead as _;
use std::process::{Command, Stdio};
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
    match spawn_daemon() {
        Ok(()) => Ok(()),
        Err(error) if is_daemon_already_running(&error) => wait_for_existing_daemon(service_name),
        Err(error) => Err(error),
    }
}
fn spawn_daemon() -> Result<()> {
    let current_exe = std::env::current_exe().context("failed to locate current executable")?;
    let mut command = Command::new(current_exe);
    command
        .arg("daemon")
        .env(READY_STDOUT_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    apply_detached_flags(&mut command);
    let mut child = command.spawn().context("failed to spawn daemon")?;
    wait_for_daemon_startup(&mut child)
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
fn wait_for_daemon_startup(child: &mut std::process::Child) -> Result<()> {
    let stdout = child
        .stdout
        .take()
        .context("daemon startup pipe was not captured")?;
    let (reply_sender, reply_receiver) = std::sync::mpsc::channel();
    let _reply_worker = std::thread::spawn(move || {
        let mut line = String::new();
        let reply = std::io::BufReader::new(stdout)
            .read_line(&mut line)
            .context("failed to read daemon startup status")
            .and_then(|bytes| parse_startup_line(bytes, &line));
        let _send_result = reply_sender.send(reply);
    });
    match reply_receiver.recv_timeout(IPC_SETUP_TIMEOUT) {
        Ok(Ok(StartupReply::Ready)) => Ok(()),
        Ok(Ok(StartupReply::AlreadyRunning { service_name })) => {
            Err(crate::runtime::daemon_lock::DaemonAlreadyRunning::new(service_name).into())
        }
        Ok(Ok(StartupReply::Failed { message })) => bail!(message),
        Ok(Err(error)) => Err(error),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => fail_daemon_startup_timeout(child),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            bail!("daemon startup reporter disconnected")
        }
    }
}
fn parse_startup_line(bytes: usize, line: &str) -> Result<StartupReply> {
    anyhow::ensure!(bytes > 0, "daemon startup reporter disconnected");
    sonic_rs::from_str(line.trim_end()).context("failed to parse daemon startup status")
}
fn fail_daemon_startup_timeout(child: &mut std::process::Child) -> Result<()> {
    if let Some(status) = child
        .try_wait()
        .context("failed to poll daemon startup status")?
    {
        bail!("daemon exited during startup with status {status}");
    }
    match child.kill() {
        Ok(()) => {
            let _status = child
                .wait()
                .context("failed to wait for daemon after startup timeout")?;
            bail!("daemon startup timed out after {IPC_SETUP_TIMEOUT:?}");
        }
        Err(kill_error) => {
            if let Some(status) = child
                .try_wait()
                .context("failed to poll daemon startup status after kill failed")?
            {
                bail!("daemon exited during startup with status {status}");
            }
            Err(kill_error).context("failed to kill daemon after startup timeout")
        }
    }
}
#[cfg(windows)]
fn apply_detached_flags(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}
#[cfg(not(windows))]
fn apply_detached_flags(_command: &mut Command) {}
