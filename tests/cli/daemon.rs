use super::{CLI_COMMAND_TIMEOUT, ChildGuard, exe};
use core::time::Duration;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Instant;
static CLI_TEST_LOCK: Mutex<()> = Mutex::new(());
static ACTIVE_CLI_ENV: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
pub(crate) struct TestGuard {
    _daemon: ChildGuard,
    _lock: MutexGuard<'static, ()>,
}
#[expect(
    clippy::missing_trait_methods,
    reason = "Drop only needs the regular destructor for this test guard"
)]
impl Drop for TestGuard {
    fn drop(&mut self) {
        ACTIVE_CLI_ENV
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}
pub(crate) fn locked() -> TestGuard {
    locked_with_env(&[])
}
pub(crate) fn locked_with_env(extra_env: &[(&str, &str)]) -> TestGuard {
    super::powershell_version::print_once();
    let guard = CLI_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let address = unused_local_address();
    let mut env = vec![("SHELL_MCP_PTY_DAEMON_ADDRESS".to_owned(), address.clone())];
    env.extend(extra_env.iter().map(|pair| {
        let key = pair.0.to_owned();
        let value = pair.1.to_owned();
        (key, value)
    }));
    let mut command = Command::new(exe());
    command
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_daemon_flags(&mut command);
    apply_env(&mut command, &env);
    {
        let mut active = ACTIVE_CLI_ENV
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *active = env;
    }
    let mut child = ChildGuard::new(command.spawn().unwrap());
    wait_for_daemon(&mut child, &address);
    TestGuard {
        _daemon: child,
        _lock: guard,
    }
}
pub(crate) fn apply_active_env(command: &mut Command) {
    let env = ACTIVE_CLI_ENV
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    apply_env(command, &env);
}
fn unused_local_address() -> String {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .to_string()
}
fn wait_for_daemon(child: &mut ChildGuard, address: &str) {
    let start = Instant::now();
    loop {
        assert!(
            child.is_running(),
            "daemon exited before accepting connections"
        );
        if ping_daemon(address) {
            return;
        }
        assert!(
            start.elapsed() < CLI_COMMAND_TIMEOUT,
            "daemon did not accept connections within {CLI_COMMAND_TIMEOUT:?}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}
fn ping_daemon(address: &str) -> bool {
    let Ok(mut stream) = TcpStream::connect(address) else {
        return false;
    };
    if stream.write_all(br#"{"kind":"ping"}"#).is_err() {
        return false;
    }
    if stream.write_all(b"\n").is_err() {
        return false;
    }
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).is_ok() && line.contains(r#""status":"ok""#)
}
fn apply_env(command: &mut Command, env: &[(String, String)]) {
    for pair in env {
        command.env(&pair.0, &pair.1);
    }
}
#[cfg(windows)]
fn apply_daemon_flags(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}
#[cfg(not(windows))]
fn apply_daemon_flags(_command: &mut Command) {}
