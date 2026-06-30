use crate::runtime::ipc::{Payload, Request, Response};
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Instant;
pub(crate) fn call(address: &str, request: &Request) -> Result<Payload> {
    let mut stream = TcpStream::connect(address)
        .with_context(|| format!("failed to connect daemon at {address}"))?;
    let line = sonic_rs::to_string(request).context("failed to serialize request")?;
    stream
        .write_all(line.as_bytes())
        .context("failed to write request")?;
    stream
        .write_all(b"\n")
        .context("failed to finish request")?;
    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .context("failed to read daemon response")?;
    let response = sonic_rs::from_str::<Response>(&response_line)
        .context("failed to parse daemon response")?;
    match response {
        Response::Ok { payload } => Ok(payload),
        Response::Err { message } => bail!(message),
    }
}
pub(crate) fn ensure_daemon(address: &str) -> Result<()> {
    if call(address, &Request::Ping).is_ok() {
        return Ok(());
    }
    let current_exe = std::env::current_exe().context("failed to locate current executable")?;
    let mut command = Command::new(current_exe);
    command
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_detached_flags(&mut command);
    command.spawn().context("failed to spawn daemon")?;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if call(address, &Request::Ping).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("daemon did not become ready within 5 seconds")
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
